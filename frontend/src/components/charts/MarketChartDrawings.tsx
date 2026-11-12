"use client";

import { useEffect, useRef, useState } from "react";
import type { PointerEvent, ReactNode } from "react";
import type { IChartApi, ISeriesApi, Time } from "lightweight-charts";

type Anchor = { time: number; price: number };
type Kind = "horizontal-ray"|"vertical-line"|"trendline"|"rectangle"|"parallel-channel"|"fibonacci"|"long-position"|"short-position";
type Tool = Kind|"select";
type Drawing = { id:string; kind:Kind; anchors:Anchor[]; color:string; width:number; dash:"solid"|"dashed"|"dotted"; label:string; visible:boolean; locked:boolean };
type Template = { name:string; instrument:string; timeframe:string; drawings:Drawing[]; showVolume:boolean; showSma:boolean };
type Props = { chart:IChartApi|null; series:ISeriesApi<"Candlestick">|null; ready:boolean; instrument:string; timeframe:string; precision:number; showVolume:boolean; showSma:boolean; onShowVolumeChange:(value:boolean)=>void; onShowSmaChange:(value:boolean)=>void };
const tools:{id:Tool;label:string;hint:string}[]=[
 {id:"select",label:"Select",hint:"Select drawings and drag their handles"},
 {id:"horizontal-ray",label:"Horizontal ray",hint:"Click once to place a price ray"},
 {id:"vertical-line",label:"Vertical line",hint:"Click once to mark a time"},
 {id:"trendline",label:"Trendline",hint:"Click two points"},
 {id:"rectangle",label:"Rectangle / zone",hint:"Click two opposite corners"},
 {id:"parallel-channel",label:"Parallel channel",hint:"Click two baseline points, then an offset point"},
 {id:"fibonacci",label:"Fibonacci",hint:"Click two swing points"},
 {id:"long-position",label:"Long position",hint:"Click entry, stop, then target"},
 {id:"short-position",label:"Short position",hint:"Click entry, stop, then target"},
];
const levels=[0,.236,.382,.5,.618,.786,1];
const colors=["#e5ad48","#4da3ff","#26a69a","#ef5350","#b38cff","#d7dce2"];
const keyFor=(i:string,t:string)=>`edgenest:drawings:v1:${i}:${t}`;
const copy=<T,>(x:T):T=>JSON.parse(JSON.stringify(x)) as T;
const count=(k:Kind)=>k==="horizontal-ray"||k==="vertical-line"?1:k==="parallel-channel"||k==="long-position"||k==="short-position"?3:2;
const timeOf=(t:number)=>t as Time;
const create=(kind:Kind,anchors:Anchor[]):Drawing=>({id:crypto.randomUUID?.()??String(Date.now()+Math.random()),kind,anchors,color:kind==="long-position"?"#26a69a":kind==="short-position"?"#ef5350":colors[0],width:2,dash:"solid",label:tools.find(x=>x.id===kind)?.label??kind,visible:true,locked:false});

export function MarketChartDrawings({chart,series,ready,instrument,timeframe,precision,showVolume,showSma,onShowVolumeChange,onShowSmaChange}:Props){
 const [tool,setTool]=useState<Tool>("select"),[drawings,setDrawings]=useState<Drawing[]>([]),[draft,setDraft]=useState<Anchor[]>([]);\n const [,setViewportVersion]=useState(0);
 const [selected,setSelected]=useState<string|null>(null),[undoStack,setUndoStack]=useState<Drawing[][]>([]),[redoStack,setRedoStack]=useState<Drawing[][]>([]);
 const [templates,setTemplates]=useState<Template[]>([]),[templateName,setTemplateName]=useState(""),[templateChoice,setTemplateChoice]=useState("");
 const [status,setStatus]=useState(""),[size,setSize]=useState({w:0,h:0}),[anchorPrice,setAnchorPrice]=useState(""),[anchorTime,setAnchorTime]=useState("");
 const [color,setColor]=useState(colors[0]),[width,setWidth]=useState("2"),[dash,setDash]=useState<Drawing["dash"]>("solid");
 const svgRef=useRef<SVGSVGElement>(null),dragRef=useRef<{id:string;index:number}|null>(null),dragBefore=useRef<Drawing[]|null>(null),hydrated=useRef(""),skipPersist=useRef(false);
 const storeKey=keyFor(instrument,timeframe),active=drawings.find(x=>x.id===selected)??null;\n useEffect(()=>{if(!chart)return;const update=()=>setViewportVersion(v=>v+1);const scale=chart.timeScale();scale.subscribeVisibleLogicalRangeChange(update);return()=>scale.unsubscribeVisibleLogicalRangeChange(update);},[chart]);

 useEffect(()=>{try{const raw=localStorage.getItem(storeKey);skipPersist.current=true;setDrawings(raw?JSON.parse(raw) as Drawing[]:[]);setSelected(null);setDraft([]);setUndoStack([]);setRedoStack([]);setTool("select");hydrated.current=storeKey;}catch(e){console.error("Drawing restore failed",e);setDrawings([]);setStatus("Could not restore drawings from browser storage.");}},[storeKey]);
 useEffect(()=>{if(hydrated.current!==storeKey)return;if(skipPersist.current){skipPersist.current=false;return;}try{localStorage.setItem(storeKey,JSON.stringify(drawings));}catch(e){console.error("Drawing persistence failed",e);setStatus("Browser storage is full or unavailable. Export drawings for backup.");}},[drawings,storeKey]);
 useEffect(()=>{try{const raw=localStorage.getItem("edgenest:drawing-templates:v1");setTemplates(raw?JSON.parse(raw) as Template[]:[]);}catch(e){console.error("Template restore failed",e);}},[]);
 useEffect(()=>{const node=svgRef.current;if(!node)return;const measure=()=>{const r=node.getBoundingClientRect();setSize({w:r.width,h:r.height});};measure();const observer=new ResizeObserver(measure);observer.observe(node);return()=>observer.disconnect();},[ready]);
 useEffect(()=>{if(!active)return;setColor(active.color);setWidth(String(active.width));setDash(active.dash);setAnchorPrice(active.anchors[0]?.price.toFixed(precision)??"");setAnchorTime(active.anchors[0]?new Date(active.anchors[0].time*1000).toISOString().slice(0,16):"");},[active,precision]);

 const commit=(next:Drawing[])=>{setUndoStack(s=>[...s.slice(-49),copy(drawings)]);setRedoStack([]);setDrawings(next);};
 const pointAt=(x:number,y:number):Anchor|null=>{if(!chart||!series)return null;const time=chart.timeScale().coordinateToTime(x),price=series.coordinateToPrice(y);if(time===null||price===null||!Number.isFinite(price))return null;const sec=typeof time==="number"?time:typeof time==="string"?Math.floor(new Date(time).getTime()/1000):Math.floor(Date.UTC(time.year,time.month-1,time.day)/1000);return{time:sec,price};};
 const xy=(a:Anchor)=>{if(!chart||!series)return null;const x=chart.timeScale().timeToCoordinate(timeOf(a.time)),y=series.priceToCoordinate(a.price);return x===null||y===null?null:{x,y};};
 const place=(p:Anchor)=>{if(tool==="select")return;const next=[...draft,p];if(next.length<count(tool)){setDraft(next);return;}commit([...drawings,create(tool,next)]);setDraft([]);setTool("select");setStatus("Drawing saved for this instrument and timeframe.");};
 const moveAnchor=(id:string,index:number,p:Anchor)=>setDrawings(current=>current.map(d=>d.id===id&&!d.locked?{...d,anchors:d.anchors.map((a,i)=>i===index?p:a)}:d));
 const onDown=(e:PointerEvent<SVGSVGElement>)=>{const el=(e.target as SVGElement).closest("[data-id]") as SVGElement|null,id=el?.getAttribute("data-id")??null,index=el?.getAttribute("data-anchor")??null;if(id){e.stopPropagation();setSelected(id);if(index!==null&&!drawings.find(d=>d.id===id)?.locked){dragRef.current={id,index:Number(index)};dragBefore.current=copy(drawings);svgRef.current?.setPointerCapture(e.pointerId);}return;}if(tool!=="select"){const r=svgRef.current!.getBoundingClientRect(),p=pointAt(e.clientX-r.left,e.clientY-r.top);if(p)place(p);}else setSelected(null);};
 const onMove=(e:React.PointerEvent<SVGSVGElement>)=>{if(!dragRef.current)return;const r=svgRef.current!.getBoundingClientRect(),p=pointAt(e.clientX-r.left,e.clientY-r.top);if(p)moveAnchor(dragRef.current.id,dragRef.current.index,p);};
 const endDrag=()=>{if(dragRef.current){dragRef.current=null;const previous=dragBefore.current;dragBefore.current=null;if(previous)setUndoStack(s=>[...s.slice(-49),previous]);setRedoStack([]);}};
 const undo=()=>{if(!undoStack.length)return;setRedoStack(s=>[...s,copy(drawings)]);setDrawings(undoStack.at(-1)!);setUndoStack(s=>s.slice(0,-1));};
 const redo=()=>{if(!redoStack.length)return;setUndoStack(s=>[...s,copy(drawings)]);setDrawings(redoStack.at(-1)!);setRedoStack(s=>s.slice(0,-1));};
 const patch=(p:Partial<Drawing>)=>{if(active)commit(drawings.map(d=>d.id===active.id?{...d,...p}:d));};
 const applyAnchor=()=>{if(!active)return;const price=Number(anchorPrice),time=new Date(anchorTime).getTime()/1000;if(!Number.isFinite(price)||price<=0||!Number.isFinite(time)){setStatus("Enter a valid positive price and UTC time.");return;}commit(drawings.map(d=>d.id===active.id?{...d,anchors:d.anchors.map((a,i)=>i===0?{price,time}:a)}:d));};
 const saveTemplate=()=>{if(!templateName.trim()){setStatus("Name the template first.");return;}const next=[...templates.filter(t=>t.name!==templateName.trim()),{name:templateName.trim(),instrument,timeframe,drawings:copy(drawings),showVolume,showSma}];try{localStorage.setItem("edgenest:drawing-templates:v1",JSON.stringify(next));setTemplates(next);setTemplateChoice(templateName.trim());setTemplateName("");setStatus("Template saved.");}catch(e){console.error(e);setStatus("Could not save template.");}};
 const loadTemplate=()=>{const t=templates.find(x=>x.name===templateChoice);if(t){commit(copy(t.drawings));onShowVolumeChange(t.showVolume);onShowSmaChange(t.showSma);setStatus(`Loaded layout “${t.name}”.`);}};
 const deleteTemplate=()=>{const next=templates.filter(x=>x.name!==templateChoice);localStorage.setItem("edgenest:drawing-templates:v1",JSON.stringify(next));setTemplates(next);setTemplateChoice("");};
 const exportFile=()=>{const blob=new Blob([JSON.stringify({version:1,instrument,timeframe,drawings},null,2)],{type:"application/json"}),url=URL.createObjectURL(blob),a=document.createElement("a");a.href=url;a.download=`edgenest-${instrument}-${timeframe}-drawings.json`;a.click();URL.revokeObjectURL(url);};
 const importFile=async(file?:File)=>{if(!file)return;try{const data=JSON.parse(await file.text());if(data.version!==1||!Array.isArray(data.drawings)||data.drawings.some((d:Drawing)=>!d.id||!Array.isArray(d.anchors)||!tools.some(t=>t.id===d.kind)))throw Error("Invalid drawing file format.");commit(data.drawings as Drawing[]);setStatus("Drawing file imported.");}catch(e){setStatus(e instanceof Error?e.message:"Import failed.");}};

 const render=(d:Drawing)=>{if(!d.visible)return null;const pts=d.anchors.map(a=>d.kind==="vertical-line"&&chart?(()=>{const x=chart.timeScale().timeToCoordinate(timeOf(a.time));return x===null?null:{x,y:0};})():xy(a));if(pts.some(p=>!p))return null;const p=pts as {x:number;y:number}[],dashArray=d.dash==="dashed"?"8 5":d.dash==="dotted"?"2 4":undefined,common={stroke:d.color,strokeWidth:d.width,strokeDasharray:dashArray,fill:"none",vectorEffect:"non-scaling-stroke" as const};let shape:ReactNode=null;
 if(d.kind==="horizontal-ray")shape=<line {...common} x1={p[0].x} x2={size.w} y1={p[0].y} y2={p[0].y}/>;
 if(d.kind==="vertical-line")shape=<line {...common} x1={p[0].x} x2={p[0].x} y1="0" y2={size.h}/>;
 if(d.kind==="trendline")shape=<line {...common} x1={p[0].x} y1={p[0].y} x2={p[1].x} y2={p[1].y}/>;
 if(d.kind==="rectangle")shape=<rect {...common} x={Math.min(p[0].x,p[1].x)} y={Math.min(p[0].y,p[1].y)} width={Math.abs(p[1].x-p[0].x)} height={Math.abs(p[1].y-p[0].y)} fill={d.color+"18"}/>;
 if(d.kind==="parallel-channel"){const ox=p[2].x-p[0].x,oy=p[2].y-p[0].y;shape=<g {...common}><line x1={p[0].x} y1={p[0].y} x2={p[1].x} y2={p[1].y}/><line x1={p[0].x+ox} y1={p[0].y+oy} x2={p[1].x+ox} y2={p[1].y+oy}/><line x1={p[0].x} y1={p[0].y} x2={p[0].x+ox} y2={p[0].y+oy}/><line x1={p[1].x} y1={p[1].y} x2={p[1].x+ox} y2={p[1].y+oy}/></g>;}
 if(d.kind==="fibonacci"){const top=Math.min(p[0].y,p[1].y),bottom=Math.max(p[0].y,p[1].y);shape=<g {...common}>{levels.map(v=>{const y=top+(bottom-top)*v;return <g key={v}><line x1={Math.min(p[0].x,p[1].x)} x2={size.w} y1={y} y2={y}/><text x={Math.min(p[0].x,p[1].x)+5} y={y-4} fill={d.color} stroke="none" fontSize="10">{(1-v).toFixed(3)}</text></g>;})}</g>;}
 if(d.kind==="long-position"||d.kind==="short-position"){const long=d.kind==="long-position",entry=p[0],stop=p[1],target=p[2],risk=Math.abs(d.anchors[0].price-d.anchors[1].price),reward=Math.abs(d.anchors[2].price-d.anchors[0].price),rr=risk? (reward/risk).toFixed(2):"—";shape=<g><rect x={entry.x} y={Math.min(entry.y,target.y)} width={Math.max(1,size.w-entry.x)} height={Math.max(1,Math.abs(entry.y-target.y))} fill="#26a69a20" stroke="#26a69a"/><rect x={entry.x} y={Math.min(entry.y,stop.y)} width={Math.max(1,size.w-entry.x)} height={Math.max(1,Math.abs(entry.y-stop.y))} fill="#ef535020" stroke="#ef5350"/><line {...common} x1={entry.x} x2={size.w} y1={entry.y} y2={entry.y}/><text x={entry.x+6} y={entry.y-7} fill={d.color} stroke="none" fontSize="11">{long?"LONG":"SHORT"} · R:R {rr}</text></g>;}
 const chosen=d.id===selected;return <g key={d.id}><g data-id={d.id} onPointerDown={e=>{e.stopPropagation();setSelected(d.id);}} style={{pointerEvents:"stroke",cursor:"pointer"}}>{shape}</g>{chosen&&!d.locked?d.anchors.map((a,i)=>{const pos=p[i];return <circle key={i} data-id={d.id} data-anchor={i} cx={pos.x} cy={pos.y} r="5" fill="#111216" stroke={d.color} strokeWidth="2" style={{pointerEvents:"all",cursor:"grab"}}/>;}):null}</g>;};

 return <section className="market-drawing-workspace">
  <div className="market-drawing-toolbar"><div className="market-drawing-tools" role="toolbar" aria-label="Chart drawing tools">{tools.map(t=><button key={t.id} type="button" title={t.hint} className={tool===t.id?"active":""} aria-pressed={tool===t.id} onClick={()=>{setTool(t.id);setDraft([]);}}>{t.label}</button>)}</div>
   <div className="market-drawing-actions"><button onClick={undo} disabled={!undoStack.length}>Undo</button><button onClick={redo} disabled={!redoStack.length}>Redo</button><button onClick={()=>{commit([]);setSelected(null);}}>Clear all</button><button onClick={exportFile}>Export</button><label className="market-import-button">Import<input type="file" accept=".json,application/json" onChange={e=>{void importFile(e.target.files?.[0]);e.currentTarget.value="";}}/></label></div></div>
  <p className="market-drawing-hint" aria-live="polite">{tool==="select"?"Select a drawing or drag its anchor handles.":tools.find(t=>t.id===tool)?.hint} {draft.length?(` · ${draft.length}/${count(tool as Kind)} points placed`):""} {status}</p>
  <div className="market-drawing-overlay-host"><svg ref={svgRef} className={`market-drawing-overlay ${tool!=="select"?"is-drawing":""}`} onPointerDown={onDown} onPointerMove={onMove} onPointerUp={endDrag} onPointerCancel={endDrag}>
   {drawings.map(render)}
   {draft.map((a,i)=>{const p=xy(a);return p?<circle key={i} cx={p.x} cy={p.y} r="4" fill="#e5ad48"/>:null;})}
  </svg></div>
  <div className="market-drawing-manager"><div className="market-drawing-list"><strong>Drawings ({drawings.length}) · {instrument} / {timeframe}</strong>{drawings.length===0?<p>No drawings yet.</p>:drawings.map(d=><div className={`market-drawing-row ${selected===d.id?"selected":""}`} key={d.id}><button className="market-drawing-name" onClick={()=>{setSelected(d.id);setTool("select");}}><i style={{background:d.color}}/>{d.label}</button><button onClick={()=>commit(drawings.map(x=>x.id===d.id?{...x,visible:!x.visible}:x))}>{d.visible?"Hide":"Show"}</button><button onClick={()=>commit(drawings.map(x=>x.id===d.id?{...x,locked:!x.locked}:x))}>{d.locked?"Unlock":"Lock"}</button><button onClick={()=>commit([...drawings,{...copy(d),id:crypto.randomUUID?.()??String(Date.now()+Math.random()),label:d.label+" copy"}])}>Duplicate</button><button onClick={()=>{commit(drawings.filter(x=>x.id!==d.id));if(selected===d.id)setSelected(null);}}>Delete</button></div>)}</div>
  {active?<div className="market-drawing-inspector"><strong>Edit drawing</strong><label>Label<input value={active.label} onChange={e=>patch({label:e.target.value})}/></label><label>Colour<input type="color" value={color} onChange={e=>{setColor(e.target.value);patch({color:e.target.value});}}/></label><label>Width<select value={width} onChange={e=>{setWidth(e.target.value);patch({width:Number(e.target.value)});}}><option>1</option><option>2</option><option>3</option><option>4</option></select></label><label>Line style<select value={dash} onChange={e=>{setDash(e.target.value as Drawing["dash"]);patch({dash:e.target.value as Drawing["dash"]});}}><option value="solid">Solid</option><option value="dashed">Dashed</option><option value="dotted">Dotted</option></select></label><label>Anchor 1 price<input value={anchorPrice} onChange={e=>setAnchorPrice(e.target.value)}/></label><label>Anchor 1 time (local)<input type="datetime-local" value={anchorTime} onChange={e=>setAnchorTime(e.target.value)}/></label><button onClick={applyAnchor}>Apply anchor</button>
   {(active.kind==="long-position"||active.kind==="short-position")&&active.anchors.length===3?<p>Risk {Math.abs(active.anchors[0].price-active.anchors[1].price).toFixed(precision)} · Reward {Math.abs(active.anchors[2].price-active.anchors[0].price).toFixed(precision)} · R:R {Math.abs(active.anchors[0].price-active.anchors[1].price)>0?(Math.abs(active.anchors[2].price-active.anchors[0].price)/Math.abs(active.anchors[0].price-active.anchors[1].price)).toFixed(2):"—"}</p>:null}
  </div>:null}</div>
  <div className="market-drawing-templates"><strong>Saved layouts & drawing templates</strong><input aria-label="Template name" placeholder="Template name" value={templateName} onChange={e=>setTemplateName(e.target.value)}/><button onClick={saveTemplate}>Save layout</button><select aria-label="Saved templates" value={templateChoice} onChange={e=>setTemplateChoice(e.target.value)}><option value="">Choose template…</option>{templates.map(t=><option key={t.name} value={t.name}>{t.name} · {t.instrument} {t.timeframe}</option>)}</select><button onClick={loadTemplate} disabled={!templateChoice}>Load</button><button onClick={deleteTemplate} disabled={!templateChoice}>Delete layout</button></div>
 </section>;
}
