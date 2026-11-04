// Execute with Node26 against the pinned source. Uses the real installed OpenAI SDK with a mock fetch; never contacts a provider.
// Usage: TS_NODE_COMPILER_OPTIONS='{"module":"CommonJS","moduleResolution":"node"}' node <script> <source-root> <output-directory>
const fs = require('fs');
const path = require('path');
const upstream = process.argv[2];
const out = process.argv[3];
require(path.join(upstream, 'node_modules/ts-node/register/transpile-only'));
const logger = { info(){}, error(){}, warn(){}, debug(){} };
const logs = { getLogger(){ return logger; } };
const Module=require('module');const original=Module._load;
let activeCase;let bodies=[];let attempts=0;let providerResponse;
const RealOpenAI=require(path.join(upstream,'node_modules/openai')).default;
class MockOpenAI extends RealOpenAI {constructor(options){super({...options,fetch:async(_url,init)=>{bodies.push(JSON.parse(init.body));attempts++;
 if(activeCase.network)throw new TypeError('fetch failed');
 const text='raw' in activeCase?activeCase.raw:JSON.stringify(activeCase.output);
 const response=activeCase.status?{error:{message:activeCase.apiError||'provider unavailable'}}:{object:'response',output:activeCase.raw===null?[]:[{type:'message',content:[{type:'output_text',text}]}]};
 providerResponse=response;return new Response(JSON.stringify(response),{status:activeCase.status||200,headers:{'content-type':'application/json','retry-after-ms':'1'}});
}});}}
Module._load=function(name,...args){return name==='openai'?MockOpenAI:original.call(this,name,...args);};
const {createAIComplianceComponent}=require(path.join(upstream,'src/adapters/ai-compliance.ts'));
const {createCommunityComplianceValidatorComponent}=require(path.join(upstream,'src/logic/community/compliance-validator.ts'));
const {communitiesErrorsHandler}=require(path.join(upstream,'src/controllers/middlewares/communities-errors.ts'));
const compliant={isCompliant:true,issues:{name:[],description:[],image:[]},confidence:0.9};
const cases=[
{name:'text',request:{name:'A community',description:'A description'},output:compliant},
{name:'image',request:{thumbnailBuffer:Buffer.from([255,216,255]),thumbnailMime:'image/jpeg'},output:compliant},
{name:'name-only',request:{name:'Name'},output:compliant},
{name:'description-only',request:{description:'Description'},output:compliant},
{name:'noncompliant',request:{name:'Rejected'},output:{isCompliant:false,issues:{name:['hate'],description:[],image:[]},confidence:0.8}},
{name:'empty-output',request:{name:'No output'},raw:null},
{name:'invalid-json',request:{name:'Invalid JSON'},raw:'not-json'},
{name:'invalid-structure',request:{name:'Bad structure'},output:{...compliant,extra:true}},
{name:'invalid-confidence',request:{name:'Bad confidence'},output:{...compliant,confidence:1.01}},
{name:'invalid-issue-count',request:{name:'Too many issues'},output:{...compliant,issues:{name:['a','b','c','d'],description:[],image:[]}}},
{name:'unicode-issue-boundary',request:{name:'Unicode boundary'},output:{...compliant,issues:{name:['😀'.repeat(25)],description:[],image:[]}}},
{name:'invalid-issue-length',request:{name:'Issue too long'},output:{...compliant,issues:{name:['😀'.repeat(26)],description:[],image:[]}}},
{name:'invalid-nested-extra',request:{name:'Nested extra'},output:{...compliant,issues:{name:[],description:[],image:[],other:[]}}},
{name:'provider-rate-limit',request:{name:'Rate limit'},status:429,apiError:'quota exhausted'},
{name:'provider-error',request:{name:'Provider failure'},status:500},
{name:'network-error',request:{name:'Network failure'},network:true},
{name:'non-production-disabled',request:{name:'No provider'},environment:'dev',dev:false,output:compliant},
{name:'non-production-enabled',request:{name:'Dev provider'},environment:'dev',dev:true,output:compliant},
{name:'cached-rejection',request:{name:'Cached rejection'},cached:{...compliant,isCompliant:false,issues:{name:['cached'],description:[],image:[]}},output:compliant},
{name:'model-key',request:{name:'Other model'},model:'fixture-model',output:compliant}
];
(async()=>{const captured=[];for(const item of cases){activeCase=item;bodies=[];attempts=0;providerResponse=undefined;let cacheKey;let cachedPut;
 const ai=await createAIComplianceComponent({config:{getString:async key=>({ENV:item.environment||'prd',OPEN_AI_MODEL:item.model||'gpt-5-nano'})[key],requireString:async()=> 'fixture-key'},logs,featureFlags:{isEnabled:()=>item.dev!==false},metrics:{increment(){},observe(){}},redis:{get:async key=>{cacheKey=key;return item.cached||null;},put:async(key,value)=>{cacheKey=key;cachedPut=value;}}});
 let result;let thrown;try{result={ok:await ai.validateCommunityContent(item.request)};}catch(error){thrown=error;result={error:error.message};}
const validator=createCommunityComplianceValidatorComponent({logs,featureFlags:{isEnabled:()=>true},aiCompliance:{validateCommunityContent:async()=>{if(thrown)throw thrown;return result.ok;}}});
const httpResult=await communitiesErrorsHandler({},async()=>{await validator.validateCommunityContent(item.request);return {status:200};});
 captured.push({name:item.name,environment:item.environment||'prd',dev:item.dev!==false,model:item.model||'gpt-5-nano',request:{...item.request,thumbnailBuffer:item.request.thumbnailBuffer?.toString('base64')},providerRequest:bodies[0],providerResponse,providerStatus:item.status||200,network:!!item.network,cached:item.cached,attempts,result,httpResult,cacheKey,cachedPut});
}fs.writeFileSync(path.join(out,'community-ai-compliance.json'),JSON.stringify({source:'decentraland/social-service-ea',revision:'b6b2cefefdf727b2ed47ddb887e77d226a41eea6',path:'src/adapters/ai-compliance.ts',node:process.version,openai:require(path.join(upstream,'node_modules/openai/package.json')).version,transport:'real installed SDK with mock fetch; no network',cases:captured},null,2)+'\n');})().catch(error=>{console.error(error);process.exitCode=1;});
