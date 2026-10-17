export type WebMetric = {metric:string;device:'mobile'|'desktop';unit:'ms'|'score';p75:number;samples:number;minimumSamples:number;assessment:'insufficient-data'|'needs-attention'|'good'|'observed'};
export type WebPerformance = {
  source:'rum';appId:string;windowHours:number;visits:number;documentVisits:number;errorVisits:number;estimatedErrorRate:number|null;
  sampleRates:number[];retentionDays:number;metrics:WebMetric[];coverage:Record<string,Record<string,number>>;
  errors:{issueId:string;title:string;occurrences:number;correlatedVisits:number;lastSeen:string}[];errorEventLimit:number;
  recent:{id:string;at:string;route:string;browser:string;device:string;navigationType:string;release:string;environment:string;
    metrics:Record<string,{value:number;unit:string}>;missing:Record<string,string>;
    diagnostics:{navigation?:Record<string,number|null>;resources:{url:string;type:string;durationMs:number;transferBytes:number|null;timingAvailable:boolean}[];longTasks:{count:number;totalMs:number;maxMs:number}}}[];
};
export type WebConfiguration = {
  appId:string;name:string;url:string;policy:'public'|'private';allowedOrigins:string[];readySelector:string|null;authProfile:string|null;
  steps:{action:'click'|'waitFor'|'assertText';selector:string;value?:string}[];intervalSeconds:number;environment:string;enabled:boolean;
  lighthouse:boolean;device:'mobile'|'desktop';alerts:boolean;failureThreshold:number;budgets?:{lcpMs?:number;tbtMs?:number;cls?:number}|null;
};
export type WebResult = {source:'synthetic';status:'passed'|'failed'|'error';ruleVersion:1;checkedAt?:string;url?:string;
  profile?:Record<string,string>;versions?:Record<string,string>;coverage?:Record<string,string|number|boolean>;errorCode?:string;
  metrics?:{name:string;unit:string;value:number;source:string}[];findings?:{rule:string;severity:'error'|'warning'|'info';evidence:string;advice:string;url:string}[]};
export type WebRun = {id:string;checkId:string;appId:string;status:'queued'|'running'|'passed'|'failed'|'error'|'cancelled';trigger:string;createdAt:string;startedAt:string|null;finishedAt:string|null;configuration:WebConfiguration;result:WebResult|null};
export type WebCheck = {id:string;version:number;configuration:WebConfiguration;health:string;consecutiveFailures:number;incidentId:string|null;createdAt:string;latestRun:{id:string;status:WebRun['status'];at:string;result:WebResult|null}|null};
export type WebCommand = {commandId:string} & (
  {action:'create';configuration:WebConfiguration}|{action:'update';id:string;expectedVersion:number;configuration:WebConfiguration}|
  {action:'delete';id:string;expectedVersion:number}|{action:'run';id:string;trigger:'manual'|'deployment'}
);
