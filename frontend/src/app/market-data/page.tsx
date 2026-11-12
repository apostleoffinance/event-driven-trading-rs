"use client";

import dynamic from "next/dynamic";
import { useQuery } from "@tanstack/react-query";
import { marketDataApi, providerStatusLabel } from "@/lib/api/market-data";
import { PageHeader } from "@/components/layout/PageHeader";
import { MarketAnalysisChartBoundary } from "@/components/charts/MarketAnalysisChartBoundary";

const MarketAnalysisChart = dynamic(
  () => import("@/components/charts/MarketAnalysisChart").then(module => module.MarketAnalysisChart),
  {
    ssr: false,
    loading: () => <section className="market-chart-state" aria-live="polite">Loading TradingView Lightweight Charts…</section>,
  },
);

export default function MarketDataPage() {
  const providers = useQuery({
    queryKey: ["market-data", "providers"],
    queryFn: () => marketDataApi.providers(),
  });

  return (
    <div className="market-data-page">
      <PageHeader title="Market Data" description="Explore market instruments, provider feeds and historical price action." />
      <MarketAnalysisChartBoundary>
        <MarketAnalysisChart />
      </MarketAnalysisChartBoundary>

      <section className="market-provider-registry">
        <div className="market-subsection-heading">
          <div><h2>Data providers</h2><p>Availability and provider-reported connection state</p></div>
          <span>{providers.data?.length ?? 0} registered</span>
        </div>
        {providers.isLoading ? <div className="market-chart-state">Loading provider registry…</div> :
          providers.error ? <div className="market-chart-state is-error" role="alert"><span>{providers.error instanceof Error ? providers.error.message : "Provider registry unavailable."}</span><button type="button" onClick={() => void providers.refetch()}>Retry registry</button></div> :
          providers.data?.length ? <div className="market-provider-grid">{providers.data.map(provider => {
            const status = providerStatusLabel(provider);
            const state = typeof provider.status === "object" && provider.status ? String((provider.status as {state?:unknown}).state ?? "unknown").toLowerCase() : "unknown";
            return <article className="market-provider-card" key={provider.id}>
              <div><strong>{provider.name ?? provider.id}</strong><span className={`market-provider-state state-${state}`}>{status}</span></div>
              <small>{provider.id}</small>
            </article>;
          })}</div> : <p className="market-empty-history">No market data providers are registered.</p>}
      </section>
    </div>
  );
}
