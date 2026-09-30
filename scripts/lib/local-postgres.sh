# A Postgres for the length of one test run.
#
#   source scripts/lib/local-postgres.sh
#   PG_CONTAINER=... PGPORT=... DBNAME=... start_local_postgres
#   # sets $DB, and $STARTED_BREW_PG when it used Homebrew
#
# Docker where there is Docker, Homebrew where there is not. macOS
# runners have neither Docker nor Postgres, and the first script to
# need one learned that the hard way; the second needed the same
# answer, and two copies of it would drift until one of them broke on
# a runner nobody was watching.
#
# Nothing here is silenced. An earlier version sent brew's output to
# /dev/null and failed four seconds in with `exit code 1` and no
# reason, which is the defect these gates exist to stop others from
# having.

start_local_postgres() {
    local container="${PG_CONTAINER:?PG_CONTAINER must be set}"
    local port="${PGPORT:?PGPORT must be set}"
    local dbname="${DBNAME:-sentori_local}"

    if command -v docker >/dev/null 2>&1 && docker info >/dev/null 2>&1; then
        docker rm -f "$container" >/dev/null 2>&1 || true
        docker run -d --name "$container" \
            -e POSTGRES_USER=postgres -e POSTGRES_PASSWORD=dev -e POSTGRES_DB=sentori \
            -p "${port}:5432" postgres:18-alpine >/dev/null
        # A real query over TCP, not `pg_isready` over the container's
        # unix socket.
        #
        # The image starts a temporary server to run its init scripts,
        # then stops it and starts the real one. `pg_isready` answers
        # yes to the temporary one, so a client that connected in the
        # gap got `57P03: the database system is starting up` and the
        # run died at "→ server" with no other explanation. Asking the
        # question the caller will ask — a query, over the port they
        # will use — is the only form of ready that means anything.
        local ready=""
        for _ in $(seq 1 90); do
            if docker exec "$container" \
                psql -U postgres -h 127.0.0.1 -d sentori -c 'SELECT 1' >/dev/null 2>&1; then
                ready=1
                break
            fi
            sleep 1
        done
        [ -n "$ready" ] || {
            echo "postgres container never answered a query; its log says:" >&2
            docker logs --tail 30 "$container" >&2 2>&1 || true
            return 1
        }
        DB="postgres://postgres:dev@127.0.0.1:${port}/sentori"
        return 0
    fi

    local formula=""
    for candidate in postgresql@18 postgresql@17 postgresql; do
        if brew list --formula "$candidate" >/dev/null 2>&1; then formula="$candidate"; break; fi
    done
    if [ -z "$formula" ]; then
        echo "  installing postgresql@17"
        brew install postgresql@17
        formula=postgresql@17
    fi
    echo "  starting $formula"
    brew services start "$formula"
    STARTED_BREW_PG="$formula"

    # `brew services` returns before the socket is up.
    local ready=""
    for _ in $(seq 1 60); do
        if pg_isready -q -h 127.0.0.1 -p 5432; then ready=1; break; fi
        sleep 1
    done
    if [ -z "$ready" ]; then
        echo "postgres never accepted connections; brew services says:" >&2
        brew services list >&2
        return 1
    fi
    dropdb --if-exists -h 127.0.0.1 "$dbname" || true
    createdb -h 127.0.0.1 "$dbname"
    # Same reason as the docker path: prove a query, not a socket.
    local answered=""
    for _ in $(seq 1 30); do
        if psql -h 127.0.0.1 -d "$dbname" -c 'SELECT 1' >/dev/null 2>&1; then
            answered=1
            break
        fi
        sleep 1
    done
    [ -n "$answered" ] || { echo "postgres accepted a connection but not a query" >&2; return 1; }
    DB="postgres://$(whoami)@127.0.0.1:5432/${dbname}"
}
