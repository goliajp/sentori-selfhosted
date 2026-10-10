import { useLocation, useParams } from "react-router-dom";
import { PublicLayout } from "../components/PublicLayout";
import { DocsView } from "../../shared/documents/DocsView";
import { allDocPages } from "../../shared/documents/model";
import { docVersions, type DocVersion } from "../../shared/documents/versions";
import changes from "../../../CHANGELOG.md?raw";
import version from "../../../VERSION?raw";
const current: DocVersion = {id:"current",version:version.trim(),releasedAt:"",commit:"",channel:"current",support:"unconfirmed",lts:null,endOfSupport:null,pages:allDocPages(),changes};
const versions = [current,...docVersions];
export default function Docs() {
  const { search, hash } = useLocation(), route = useParams();
  const params = new URLSearchParams(search);
  params.set("topic",route["*"] ?? "");
  if (hash) { try { params.set("anchor",decodeURIComponent(hash.slice(1))); } catch { params.set("anchor",hash.slice(1)); } }
  return <PublicLayout><DocsView params={params} versions={versions} defaultVersion={current}/></PublicLayout>;
}
