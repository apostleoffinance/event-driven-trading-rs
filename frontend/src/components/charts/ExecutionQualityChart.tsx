"use client";
import dynamic from "next/dynamic";
import { useMemo } from "react";
import { useTheme } from "@/features/theme/ThemeProvider";

const ReactECharts=dynamic(()=>import("echarts-for-react"),{ssr:false});
function css(name:string,fallback:string){if(typeof window==="undefined")return fallback;return getComputedStyle(document.documentElement).getPropertyValue(name).trim()||fallback;}

export function ExecutionQualityChart({mfe,mae,averageR}:{mfe:string|null|undefined;mae:string|null|undefined;averageR:string|null|undefined}){
 const {theme}=useTheme();
 const option=useMemo(()=>{
  const positive=css("--positive",theme==="light"?"#1a7f4b":"#4fd4a1");
  const negative=css("--negative",theme==="light"?"#c6283a":"#f07176");
  const muted=css("--chart-dim","#a8abb2"),grid=css("--chart-grid","#2a2d34");
  const values=[mfe==null?null:Number(mfe),mae==null?null:-Math.abs(Number(mae)),averageR==null?null:Number(averageR)];
  const labels=["Avg MFE (R)","Avg MAE (R)","Avg realized R"];
  return {backgroundColor:"transparent",grid:{left:42,right:18,top:24,bottom:38},tooltip:{trigger:"axis",formatter:(items:Array<{dataIndex:number;value:number|null}>)=>{const i=items[0]?.dataIndex??0;const v=values[i];return labels[i]+": "+(v==null?"Not recorded":v.toFixed(2)+"R");}},xAxis:{type:"category",data:labels,axisLabel:{color:muted,interval:0},axisLine:{lineStyle:{color:grid}}},yAxis:{type:"value",axisLabel:{color:muted,formatter:(v:number)=>v+"R"},splitLine:{lineStyle:{color:grid}},axisLine:{show:false}},series:[{type:"bar",barMaxWidth:48,data:values.map(v=>({value:v,itemStyle:{color:v==null?grid:v>=0?positive:negative,borderRadius:v!=null&&v>=0?[4,4,0,0]:[0,0,4,4]}})),label:{show:true,position:"top",color:muted,formatter:(p:{value:number|null})=>p.value==null?"—":Number(p.value).toFixed(2)+"R"}}]};
 },[mfe,mae,averageR,theme]);
 return <ReactECharts option={option} style={{height:280,width:"100%"}} opts={{renderer:"canvas"}} notMerge />;
}
