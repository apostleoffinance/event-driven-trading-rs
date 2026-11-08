"use client";

import { Component, type ErrorInfo, type ReactNode } from "react";

type Props = { children: ReactNode };
type State = { hasError: boolean };

export class MarketAnalysisChartBoundary extends Component<Props, State> {
  state: State = { hasError: false };

  static getDerivedStateFromError(): State {
    return { hasError: true };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    // Keep a diagnostic in browser DevTools without taking down the rest of Market Data.
    console.error("[EdgeNest] Market Analysis chart failed to initialize.", error, info.componentStack);
  }

  render() {
    if (this.state.hasError) {
      return (
        <section className="panel error" role="alert">
          <h2>Market Analysis chart unavailable</h2>
          <p>The chart failed to initialize. The rest of Market Data remains available.</p>
          <button className="btn" type="button" onClick={() => this.setState({ hasError: false })}>
            Retry chart
          </button>
        </section>
      );
    }

    return this.props.children;
  }
}
