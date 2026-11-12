"use client";

import { Component, type ErrorInfo, type ReactNode } from "react";

type Props = { children: ReactNode };
type State = { error: Error | null; retryKey: number };

export class MarketAnalysisChartBoundary extends Component<Props, State> {
  state: State = { error: null, retryKey: 0 };

  static getDerivedStateFromError(error: Error): Partial<State> {
    return { error };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error("[EdgeNest] Market Analysis chart crashed.", error, info.componentStack);
  }

  render() {
    if (this.state.error) {
      return (
        <section className="market-chart-state is-error" role="alert">
          <strong>Market Analysis could not initialize</strong>
          <span>The chart encountered a browser rendering error. Other Market Data features remain available.</span>
          <details><summary>Technical diagnostic</summary><pre>{this.state.error.message}</pre></details>
          <button type="button" onClick={() => this.setState(state => ({ error: null, retryKey: state.retryKey + 1 }))}>Retry chart</button>
        </section>
      );
    }
    return <div key={this.state.retryKey}>{this.props.children}</div>;
  }
}
