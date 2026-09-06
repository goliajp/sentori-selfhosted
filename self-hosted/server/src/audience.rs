//! Who a push is for, as one expression.
//!
//! Three ways of naming an audience were on the table:
//!
//! 1. by app user id — "notify usr_123"
//! 2. by attribute equality — "notify everyone on the pro plan in Japan"
//! 3. by a full expression — "plan in (pro, team) and version ≥ 4.2"
//!
//! They are not three features. The first is one leaf of the third and
//! the second is a conjunction of leaves, so all three are the same
//! engine with two shorthands in front of it. Written as three code
//! paths they would drift: the version that handles `revoked_at`, the
//! version that caps the audience, and the version that does neither.
//!
//! ## What a leaf can name
//!
//! Two namespaces, because the motivating request needs both. "The
//! Japanese pro users on 4.2.0" mixes a fact about the person (`plan`,
//! from `sentori.user()`) with a fact about the build (`appVersion`,
//! from `register()`), and those arrive from different callers at
//! different times. Collapsing them into one bag would make a send
//! aimed at `plan = pro` match a device whose build channel was called
//! `pro`.
//!
//! ```json
//! { "all": [
//!     { "trait": "plan", "in": ["pro", "team"] },
//!     { "device": "appVersion", "versionGte": "4.2" },
//!     { "any": [ { "trait": "locale", "is": "ja-JP" },
//!                { "trait": "org", "is": "acme" } ] } ] }
//! ```
//!
//! ## A tree, not a string
//!
//! The dashboard's condition editor builds and reads a structure. A
//! string grammar would mean writing a parser, then writing the same
//! structure behind it anyway to render the editor — and every syntax
//! error would arrive as a column number in someone else's process.
//! The rendering in the docs (`plan IN (...) AND ...`) is what the
//! editor draws, not what crosses the wire.

use serde_json::{Map, Value};

/// How deep a caller may nest, and how many leaves in total.
///
/// Not a safety limit against an attacker — this endpoint needs an
/// admin token. It is a limit against a generated tree: a dashboard
/// bug that appends to the wrong node produces a query Postgres will
/// try very hard to plan, and the failure is then a stuck request
/// rather than a rejected one.
const MAX_DEPTH: usize = 8;
const MAX_NODES: usize = 64;

/// A value bound into the compiled statement, in the order the
/// placeholders appear.
///
/// The compiler never puts a caller's value into the SQL text — trait
/// names included, which is why key lookups are `-> ($n)::text` rather
/// than the literal-looking form.
#[derive(Debug)]
pub enum Bind {
    Text(String),
    Json(Value),
    Uuid(uuid::Uuid),
}

impl Bind {
    /// Attach one bind to a query, whatever it holds.
    ///
    /// The caller loops over the binds in order and never matches on
    /// them, so adding a kind cannot leave a call site behind.
    pub fn attach<'q>(
        &'q self,
        q: sqlx::query::Query<'q, sqlx::Postgres, sqlx::postgres::PgArguments>,
    ) -> sqlx::query::Query<'q, sqlx::Postgres, sqlx::postgres::PgArguments> {
        match self {
            Bind::Text(t) => q.bind(t),
            Bind::Json(v) => q.bind(v),
            Bind::Uuid(u) => q.bind(u),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Source {
    Trait,
    Device,
}

impl Source {
    /// The column, already `NOT NULL`-safe. `metadata` predates its
    /// default and old rows can hold SQL NULL; `COALESCE` costs
    /// nothing and keeps `NOT (...)` from turning into NULL — which
    /// would drop exactly the rows a negation is supposed to keep.
    fn column(self) -> &'static str {
        match self {
            Source::Trait => "COALESCE(dt.traits, '{}'::jsonb)",
            Source::Device => "COALESCE(dt.metadata, '{}'::jsonb)",
        }
    }
}

enum Cmp {
    Gte,
    Gt,
    Lte,
    Lt,
}

impl Cmp {
    fn op(&self) -> &'static str {
        match self {
            Cmp::Gte => ">=",
            Cmp::Gt => ">",
            Cmp::Lte => "<=",
            Cmp::Lt => "<",
        }
    }
}

enum Node {
    All(Vec<Node>),
    Any(Vec<Node>),
    Not(Box<Node>),
    /// A raw app user id, hashed the way the device hashed it.
    UserKey(String),
    /// Everyone this issue happened to.
    ///
    /// `issue_user_hits` is written at ingest and carries the same
    /// hash the device row does, so "notify the people who hit this"
    /// is a join rather than a list somebody has to assemble. It was
    /// the one thing the whole design pointed at and could not do.
    Issue(uuid::Uuid),
    /// The key is present at all, whatever its value.
    Exists {
        source: Source,
        key: String,
    },
    /// Equal to this exact value.
    Is {
        source: Source,
        key: String,
        value: Value,
    },
    /// Textually starts with this.
    Prefix {
        source: Source,
        key: String,
        value: String,
    },
    /// Compared as a number.
    Number {
        source: Source,
        key: String,
        cmp: Cmp,
        value: f64,
    },
    /// Compared as a version.
    Version {
        source: Source,
        key: String,
        cmp: Cmp,
        value: String,
    },
    /// Matches nothing, and says so rather than matching everything.
    ///
    /// The only producer is an `any` with no branches. An empty
    /// disjunction is false; the reason it is a node rather than an
    /// error is that a dashboard editor holds one the moment someone
    /// adds an "or" group before filling it in.
    Never,
}

pub struct Audience {
    root: Node,
}

impl Audience {
    /// The `WHERE` fragment, with placeholders numbered from `next`.
    ///
    /// Returns the fragment and its binds in the same order, which the
    /// caller feeds to `sqlx` in a loop. Both halves come from one
    /// walk so they cannot disagree about how many there are.
    #[must_use]
    pub fn to_sql(&self, next: usize) -> (String, Vec<Bind>) {
        let mut binds = Vec::new();
        let mut n = next;
        let sql = compile(&self.root, &mut n, &mut binds);
        (sql, binds)
    }
}

/// Read an audience from the request body.
///
/// `app_user_id` and `traits` are the two shorthands; `audience` is
/// the expression. Giving more than one is an error rather than an
/// intersection: a caller who sets both meant one of them, and
/// guessing which produces a send that goes somewhere plausible and
/// wrong.
///
/// `Ok(None)` means the body named no audience at all — the caller
/// is targeting by token or topic and this module has no opinion.
pub fn from_request(
    app_user_id: Option<&str>,
    traits: Option<&Value>,
    audience: Option<&Value>,
) -> Result<Option<Audience>, String> {
    let given = usize::from(app_user_id.is_some())
        + usize::from(traits.is_some())
        + usize::from(audience.is_some());
    if given > 1 {
        return Err("give one of appUserId, traits or audience — not several; \
                    to combine them, write them as one audience expression"
            .to_string());
    }

    if let Some(id) = app_user_id {
        let Some(key) = crate::identity::user_key_for_app_user_id(id) else {
            return Err("appUserId is empty".to_string());
        };
        return Ok(Some(Audience {
            root: Node::UserKey(key),
        }));
    }

    if let Some(t) = traits {
        let Some(map) = t.as_object() else {
            return Err("traits must be an object of attribute to value".to_string());
        };
        if map.is_empty() {
            // Every device in the project, from a field that reads like
            // a filter. Whatever the caller meant, they did not mean
            // that.
            return Err("traits is empty, which would match every device; \
                        omit it to target by token or topic"
                .to_string());
        }
        let leaves = map
            .iter()
            .map(|(k, v)| Node::Is {
                source: Source::Trait,
                key: k.clone(),
                value: v.clone(),
            })
            .collect();
        return Ok(Some(Audience {
            root: Node::All(leaves),
        }));
    }

    match audience {
        Some(v) => {
            let mut budget = MAX_NODES;
            let root = parse(v, 0, &mut budget)?;
            Ok(Some(Audience { root }))
        }
        None => Ok(None),
    }
}

fn parse(v: &Value, depth: usize, budget: &mut usize) -> Result<Node, String> {
    if depth > MAX_DEPTH {
        return Err(format!("audience nests deeper than {MAX_DEPTH} levels"));
    }
    if *budget == 0 {
        return Err(format!("audience has more than {MAX_NODES} conditions"));
    }
    *budget -= 1;

    let Some(obj) = v.as_object() else {
        return Err("each audience condition must be an object".to_string());
    };

    if let Some(list) = obj.get("all") {
        return Ok(Node::All(parse_branches(list, depth, budget, "all")?));
    }
    if let Some(list) = obj.get("any") {
        let branches = parse_branches(list, depth, budget, "any")?;
        return Ok(if branches.is_empty() {
            Node::Never
        } else {
            Node::Any(branches)
        });
    }
    if let Some(inner) = obj.get("not") {
        return Ok(Node::Not(Box::new(parse(inner, depth + 1, budget)?)));
    }
    if let Some(id) = obj.get("user") {
        let Some(id) = id.as_str() else {
            return Err("user must be the app's user id, as a string".to_string());
        };
        let Some(key) = crate::identity::user_key_for_app_user_id(id) else {
            return Err("user is empty".to_string());
        };
        return Ok(Node::UserKey(key));
    }
    if let Some(id) = obj.get("issue") {
        let parsed = id.as_str().and_then(|s| s.parse::<uuid::Uuid>().ok());
        let Some(parsed) = parsed else {
            return Err("issue must be an issue id".to_string());
        };
        return Ok(Node::Issue(parsed));
    }
    if let Some(k) = obj.get("userKey") {
        let Some(k) = k.as_str().filter(|s| !s.is_empty()) else {
            return Err("userKey must be a non-empty string".to_string());
        };
        return Ok(Node::UserKey(k.to_string()));
    }

    parse_leaf(obj, depth, budget)
}

fn parse_branches(
    list: &Value,
    depth: usize,
    budget: &mut usize,
    name: &str,
) -> Result<Vec<Node>, String> {
    let Some(items) = list.as_array() else {
        return Err(format!("{name} must be a list of conditions"));
    };
    items
        .iter()
        .map(|i| parse(i, depth + 1, budget))
        .collect::<Result<Vec<_>, _>>()
}

fn parse_leaf(obj: &Map<String, Value>, depth: usize, budget: &mut usize) -> Result<Node, String> {
    let (source, key) = match (obj.get("trait"), obj.get("device")) {
        (Some(_), Some(_)) => {
            return Err("a condition names either a trait or a device field, not both".to_string());
        }
        (Some(k), None) => (Source::Trait, k),
        (None, Some(k)) => (Source::Device, k),
        (None, None) => {
            let named: Vec<&str> = obj.keys().map(String::as_str).collect();
            return Err(format!(
                "condition names none of all, any, not, user, issue, trait or device — \
             got {named:?}"
            ));
        }
    };
    let Some(key) = key.as_str().filter(|s| !s.is_empty()) else {
        return Err("the attribute name must be a non-empty string".to_string());
    };
    let key = key.to_string();

    // `in` is written out rather than compiled: a disjunction of
    // equalities is what it means, and the equalities are already
    // index-friendly. One less SQL form to get right.
    if let Some(list) = obj.get("in") {
        let Some(items) = list.as_array() else {
            return Err("in must be a list of values".to_string());
        };
        if items.is_empty() {
            return Ok(Node::Never);
        }
        if *budget < items.len() {
            return Err(format!("audience has more than {MAX_NODES} conditions"));
        }
        *budget -= items.len();
        if depth + 1 > MAX_DEPTH {
            return Err(format!("audience nests deeper than {MAX_DEPTH} levels"));
        }
        return Ok(Node::Any(
            items
                .iter()
                .map(|v| Node::Is {
                    source,
                    key: key.clone(),
                    value: v.clone(),
                })
                .collect(),
        ));
    }

    parse_comparison(obj, source, key)
}

/// Which comparison a leaf is asking for.
///
/// Split from `parse_leaf` only for length; the two halves are one
/// decision — `parse_leaf` resolves *what* is being compared, this
/// resolves *how*.
fn parse_comparison(obj: &Map<String, Value>, source: Source, key: String) -> Result<Node, String> {
    if let Some(v) = obj.get("is") {
        return Ok(Node::Is {
            source,
            key,
            value: v.clone(),
        });
    }
    if let Some(v) = obj.get("isNot") {
        return Ok(Node::Not(Box::new(Node::Is {
            source,
            key,
            value: v.clone(),
        })));
    }
    if let Some(v) = obj.get("exists") {
        let Some(want) = v.as_bool() else {
            return Err("exists must be true or false".to_string());
        };
        let leaf = Node::Exists { source, key };
        return Ok(if want {
            leaf
        } else {
            Node::Not(Box::new(leaf))
        });
    }
    if let Some(v) = obj.get("prefix") {
        let Some(s) = v.as_str() else {
            return Err("prefix must be a string".to_string());
        };
        return Ok(Node::Prefix {
            source,
            key,
            value: s.to_string(),
        });
    }

    for (name, cmp) in [
        ("versionGte", Cmp::Gte),
        ("versionGt", Cmp::Gt),
        ("versionLte", Cmp::Lte),
        ("versionLt", Cmp::Lt),
    ] {
        if let Some(v) = obj.get(name) {
            let Some(s) = v.as_str().filter(|s| !s.is_empty()) else {
                return Err(format!("{name} must be a version string like \"4.2.0\""));
            };
            return Ok(Node::Version {
                source,
                key,
                cmp,
                value: s.to_string(),
            });
        }
    }

    for (name, cmp) in [
        ("gte", Cmp::Gte),
        ("gt", Cmp::Gt),
        ("lte", Cmp::Lte),
        ("lt", Cmp::Lt),
    ] {
        if let Some(v) = obj.get(name) {
            let Some(n) = v.as_f64() else {
                return Err(format!(
                    "{name} must be a number — to compare versions use version{}",
                    name[..1].to_uppercase() + &name[1..]
                ));
            };
            return Ok(Node::Number {
                source,
                key,
                cmp,
                value: n,
            });
        }
    }

    Err(format!(
        "condition on {key:?} has no comparison — expected one of is, isNot, in, \
         exists, prefix, gte/gt/lte/lt or versionGte/versionGt/versionLte/versionLt"
    ))
}

fn compile(node: &Node, n: &mut usize, binds: &mut Vec<Bind>) -> String {
    match node {
        Node::All(items) if items.is_empty() => "true".to_string(),
        Node::All(items) => join(items, " AND ", n, binds),
        Node::Any(items) => join(items, " OR ", n, binds),
        Node::Not(inner) => format!("NOT ({})", compile(inner, n, binds)),
        Node::Never => "false".to_string(),

        Node::UserKey(key) => {
            let p = take(n, binds, Bind::Text(key.clone()));
            format!("dt.user_key = ({p})::text")
        }

        // Joined through `issues` rather than straight to the hits, so
        // an issue id from another project selects nothing. Without
        // that the id is the only thing checked, and two apps that
        // share a person would share an audience.
        Node::Issue(id) => {
            let p = take(n, binds, Bind::Uuid(*id));
            format!(
                "EXISTS (SELECT 1 FROM issue_user_hits ih                  JOIN issues i ON i.id = ih.issue_id                  WHERE ih.issue_id = ({p})::uuid                  AND i.project_id = dt.project_id                  AND ih.user_key = dt.user_key)"
            )
        }

        Node::Exists { source, key } => {
            let p = take(n, binds, Bind::Text(key.clone()));
            format!("{} -> ({p})::text IS NOT NULL", source.column())
        }

        // Containment when the value is a scalar, because that is what
        // the GIN index answers. For an object or an array it is *not*
        // equality — `@>` on an array means "contains these elements" —
        // so those compile to a plain comparison, which is slower and
        // correct rather than fast and subtly wrong.
        Node::Is { source, key, value } if value.is_object() || value.is_array() => {
            let k = take(n, binds, Bind::Text(key.clone()));
            let v = take(n, binds, Bind::Json(value.clone()));
            format!("{} -> ({k})::text = ({v})::jsonb", source.column())
        }
        Node::Is { source, key, value } => {
            let mut one = Map::new();
            one.insert(key.clone(), value.clone());
            let p = take(n, binds, Bind::Json(Value::Object(one)));
            format!("{} @> ({p})::jsonb", source.column())
        }

        Node::Prefix { source, key, value } => {
            let k = take(n, binds, Bind::Text(key.clone()));
            let v = take(n, binds, Bind::Text(value.clone()));
            // `starts_with` is NULL when the key is absent, and a NULL
            // in a conjunction is not a match — which is the answer.
            format!(
                "COALESCE(starts_with({} ->> ({k})::text, ({v})::text), false)",
                source.column()
            )
        }

        // A cast on text nobody validated aborts the whole statement,
        // so the shape of the value is checked before it is cast. The
        // guard is a `CASE`, whose arms Postgres does evaluate in
        // order — a `WHERE ... AND (...)::numeric` would not be safe
        // the same way.
        Node::Number {
            source,
            key,
            cmp,
            value,
        } => {
            let col = source.column();
            let k = take(n, binds, Bind::Text(key.clone()));
            let v = take(n, binds, Bind::Text(value.to_string()));
            format!(
                "CASE WHEN {col} ->> ({k})::text ~ '^-?[0-9]+(\\.[0-9]+)?$' \
                 THEN ({col} ->> ({k})::text)::numeric {} ({v})::numeric \
                 ELSE false END",
                cmp.op()
            )
        }

        // `sentori_version_key` returns NULL for anything it cannot
        // read, so an unparseable version compares to NULL and the row
        // is left out — rather than being swept in by a text
        // comparison that puts 4.10 before 4.2.
        Node::Version {
            source,
            key,
            cmp,
            value,
        } => {
            let col = source.column();
            let k = take(n, binds, Bind::Text(key.clone()));
            let v = take(n, binds, Bind::Text(value.clone()));
            format!(
                "COALESCE(sentori_version_key({col} ->> ({k})::text) {} \
                 sentori_version_key(({v})::text), false)",
                cmp.op()
            )
        }
    }
}

fn join(items: &[Node], sep: &str, n: &mut usize, binds: &mut Vec<Bind>) -> String {
    let parts: Vec<String> = items.iter().map(|i| compile(i, n, binds)).collect();
    format!("({})", parts.join(sep))
}

fn take(n: &mut usize, binds: &mut Vec<Bind>, bind: Bind) -> String {
    let p = format!("${n}");
    *n += 1;
    binds.push(bind);
    p
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sql_of(v: &Value) -> Result<(String, usize), String> {
        let a = from_request(None, None, Some(v))?.ok_or("no audience")?;
        let (sql, binds) = a.to_sql(2);
        Ok((sql, binds.len()))
    }

    /// Every placeholder the fragment mentions has a bind behind it,
    /// numbered from where the caller said to start.
    ///
    /// The two halves come from one walk, so this is really a check
    /// that the walk never emits SQL without pushing — which is the
    /// bug that ends as "bind message supplies 3 parameters, but
    /// prepared statement requires 4" at run time, on a query nothing
    /// tested because it needed a database.
    #[test]
    fn placeholders_and_binds_agree() {
        let v = json!({ "all": [
            { "trait": "plan", "in": ["pro", "team"] },
            { "device": "appVersion", "versionGte": "4.2" },
            { "any": [ { "trait": "locale", "is": "ja-JP" },
                       { "trait": "org", "exists": true } ] },
            { "not": { "trait": "churned", "is": true } },
            { "device": "buildNumber", "gte": 41.0 },
            { "trait": "email", "prefix": "ops@" } ] });
        let probe = sql_of(&v);
        assert!(
            probe.is_ok(),
            "a well-formed audience did not compile: {probe:?}"
        );
        let Ok((sql, count)) = probe else { return };
        let mut seen: Vec<usize> = sql
            .split('$')
            .skip(1)
            .filter_map(|s| {
                s.chars()
                    .take_while(char::is_ascii_digit)
                    .collect::<String>()
                    .parse()
                    .ok()
            })
            .collect();
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(
            seen,
            (2..2 + count).collect::<Vec<_>>(),
            "the fragment's placeholders are not exactly the binds it produced: {sql}"
        );
    }

    /// No caller value reaches the SQL text.
    ///
    /// Trait names are the interesting half — they are the one part of
    /// a condition that names a column-like thing, and the tempting
    /// way to write it is to paste the name in.
    #[test]
    fn nothing_a_caller_wrote_appears_in_the_sql() {
        let v = json!({ "all": [
            { "trait": "plan'; DROP TABLE device_tokens; --", "is": "pro" },
            { "device": "chan", "prefix": "'; DELETE FROM push_sends; --" } ] });
        let probe = sql_of(&v);
        assert!(
            probe.is_ok(),
            "a hostile-looking but well-formed audience did not compile: {probe:?}"
        );
        let Ok((sql, _)) = probe else { return };
        assert!(
            !sql.contains("DROP") && !sql.contains("DELETE") && !sql.contains("plan"),
            "a caller's text reached the statement: {sql}"
        );
    }

    /// An `any` with nothing in it is false, not true.
    ///
    /// This is the difference between "the editor has an empty or-group
    /// in it" and "send this to everyone who has ever installed the
    /// app", and the two are one `join` of an empty list apart.
    #[test]
    fn an_empty_disjunction_matches_nothing() {
        let probe = sql_of(&json!({ "any": [] }));
        assert!(probe.is_ok(), "an empty any did not compile: {probe:?}");
        let Ok((sql, _)) = probe else { return };
        assert_eq!(sql, "false");

        let probe = sql_of(&json!({ "trait": "plan", "in": [] }));
        assert!(probe.is_ok(), "an empty in did not compile: {probe:?}");
        let Ok((sql, _)) = probe else { return };
        assert_eq!(sql, "false");
    }

    /// The shorthands are the expression, not a second implementation.
    #[test]
    fn the_shorthands_compile_to_the_same_thing_as_the_long_form() {
        let by_sugar = from_request(Some("usr_123"), None, None)
            .ok()
            .flatten()
            .map(|a| a.to_sql(2).0);
        let by_expression = from_request(None, None, Some(&json!({ "user": "usr_123" })))
            .ok()
            .flatten()
            .map(|a| a.to_sql(2).0);
        assert!(
            by_sugar.is_some() && by_sugar == by_expression,
            "appUserId and the equivalent expression produced different SQL: \
             {by_sugar:?} vs {by_expression:?}"
        );

        let traits = json!({ "plan": "pro" });
        let by_sugar = from_request(None, Some(&traits), None)
            .ok()
            .flatten()
            .map(|a| a.to_sql(2));
        let by_expression = from_request(
            None,
            None,
            Some(&json!({ "all": [ { "trait": "plan", "is": "pro" } ] })),
        )
        .ok()
        .flatten()
        .map(|a| a.to_sql(2));
        assert!(
            by_sugar.is_some()
                && by_sugar.as_ref().map(|(s, _)| s) == by_expression.as_ref().map(|(s, _)| s),
            "traits and the equivalent expression produced different SQL"
        );
    }

    /// The id is hashed before it is compared, because the column is a
    /// hash. Comparing the raw id would match nothing, for every user,
    /// and report success.
    #[test]
    fn an_app_user_id_is_hashed_on_the_way_in() {
        let probe = from_request(Some("usr_123"), None, None).ok().flatten();
        assert!(
            probe.is_some(),
            "an app user id did not produce an audience"
        );
        let Some(a) = probe else { return };
        let (_, binds) = a.to_sql(2);
        let bound = binds.iter().find_map(|b| match b {
            Bind::Text(t) => Some(t.clone()),
            Bind::Json(_) | Bind::Uuid(_) => None,
        });
        assert_eq!(
            bound,
            crate::identity::user_key_for_app_user_id("usr_123"),
            "the raw id was bound where the hash belongs"
        );
    }

    /// A filter that matches everything is refused where it would be
    /// read as a filter.
    #[test]
    fn an_empty_traits_object_is_refused() {
        let empty = json!({});
        assert!(from_request(None, Some(&empty), None).is_err());
    }

    /// Two targeting modes at once is a caller who meant one of them.
    #[test]
    fn combining_the_shorthands_is_refused_rather_than_guessed_at() {
        let traits = json!({ "plan": "pro" });
        assert!(from_request(Some("usr_1"), Some(&traits), None).is_err());
    }

    #[test]
    fn a_tree_that_is_too_deep_or_too_wide_is_refused() {
        let mut deep = json!({ "trait": "a", "is": 1 });
        for _ in 0..MAX_DEPTH + 2 {
            deep = json!({ "all": [deep] });
        }
        assert!(sql_of(&deep).is_err(), "an over-deep audience compiled");

        let wide: Vec<Value> = (0..MAX_NODES + 5)
            .map(|i| json!({ "trait": format!("k{i}"), "is": 1 }))
            .collect();
        assert!(
            sql_of(&json!({ "all": wide })).is_err(),
            "an over-wide audience compiled"
        );
    }

    /// The join that "notify everyone who hit this" needs, and the
    /// one thing the design pointed at and could not do.
    #[test]
    fn an_issue_selects_the_people_it_happened_to() {
        let id = "019ff080-2aeb-7e30-aba1-4431b296d120";
        let probe = sql_of(&json!({ "issue": id }));
        assert!(
            probe.is_ok(),
            "an issue audience did not compile: {probe:?}"
        );
        let Ok((sql, count)) = probe else { return };
        assert_eq!(count, 1, "the issue id has to be bound, not pasted");
        assert!(
            sql.contains("issue_user_hits") && sql.contains("i.project_id = dt.project_id"),
            "an issue audience must join through issues, or an id from another \
             project selects devices here: {sql}"
        );
        assert!(
            !sql.contains(id),
            "the id reached the statement text: {sql}"
        );
    }

    /// Anything that is not an id is refused rather than compiled into
    /// a condition that matches nobody and looks like it worked.
    #[test]
    fn an_issue_that_is_not_an_id_is_refused() {
        assert!(sql_of(&json!({ "issue": "the login crash" })).is_err());
        assert!(sql_of(&json!({ "issue": 12 })).is_err());
    }

    /// A named list and a device condition in one expression.
    ///
    /// The three shorthands cannot be combined — `appUserId` with
    /// `traits` is refused — and that led at least one reader to
    /// believe "these forty people, but only the ones on 4.2" was not
    /// expressible. It is: the restriction is on the sugar, not on the
    /// expression, and `user` is a leaf like any other.
    #[test]
    fn a_backend_list_can_be_narrowed_by_a_device_condition() {
        let v = json!({ "all": [
            { "any": [ { "user": "usr_1" }, { "user": "usr_2" } ] },
            { "device": "appVersion", "versionGte": "4.2" },
            { "trait": "plan", "is": "pro" } ] });
        let probe = sql_of(&v);
        assert!(
            probe.is_ok(),
            "a list narrowed by a condition did not compile: {probe:?}"
        );
        let Ok((sql, count)) = probe else { return };
        // Two ids, then the version leaf binds both its key and its
        // value, then the trait binds one object for containment.
        assert_eq!(count, 5, "the leaves did not bind what they compile to");
        assert!(
            sql.contains("dt.user_key") && sql.contains("sentori_version_key"),
            "the identity and the version are not both in the clause: {sql}"
        );
    }

    /// A number where a version belongs is the mistake worth naming:
    /// `gte: 4.2` on "4.10.0" reads 4.10 as four-point-one.
    #[test]
    fn a_numeric_comparison_on_a_version_is_pointed_at_the_version_operator() {
        let err = sql_of(&json!({ "device": "appVersion", "gte": "4.2" })).err();
        assert!(
            err.as_ref().is_some_and(|e| e.contains("versionGte")),
            "the error did not name the operator that would have worked — got {err:?}"
        );
    }

    /// A condition with no comparison names what it could have been.
    #[test]
    fn an_unrecognised_condition_says_what_was_expected() {
        let err = sql_of(&json!({ "trait": "plan", "equals": "pro" })).err();
        assert!(
            err.as_ref().is_some_and(|e| e.contains("is")),
            "got {err:?}"
        );
    }

    /// An array value does not compile to containment, which would
    /// match a superset rather than the value asked for.
    #[test]
    fn an_array_value_is_compared_rather_than_contained() {
        let probe = sql_of(&json!({ "trait": "tags", "is": ["a"] }));
        assert!(probe.is_ok(), "an array value did not compile: {probe:?}");
        let Ok((sql, _)) = probe else { return };
        assert!(
            !sql.contains("@>"),
            "an array compiled to containment, which matches rows holding more \
             than what was asked for: {sql}"
        );
    }
}
