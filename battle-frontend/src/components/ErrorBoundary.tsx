import React, { Component } from 'react';
import type { ErrorInfo, ReactNode } from 'react';

interface Props {
  children?: ReactNode;
}

interface State {
  hasError: boolean;
  error: Error | null;
}

export class ErrorBoundary extends Component<Props, State> {
  public state: State = {
    hasError: false,
    error: null
  };

  public static getDerivedStateFromError(error: Error): State {
    return { hasError: true, error };
  }

  public componentDidCatch(error: Error, errorInfo: ErrorInfo) {
    console.error('Uncaught error:', error, errorInfo);
  }

  public render() {
    if (this.state.hasError) {
      return (
        <div className="flex flex-col items-center justify-center min-h-screen bg-kaspa-dark text-white p-8 text-center font-sans">
          <div className="text-5xl mb-6">⚠️</div>
          <h1 className="text-2xl font-bold text-red-500 mb-4">Ein unerwarteter Fehler ist aufgetreten</h1>
          <p className="text-gray-400 max-w-2xl mb-8 break-words text-left bg-black/30 p-4 rounded-lg border border-red-500/20 font-mono text-sm shadow-inner">
            {this.state.error?.message || 'Unbekannter Fehler'}
          </p>
          <button
            onClick={() => window.location.reload()}
            className="px-6 py-3 bg-red-600 hover:bg-red-500 text-white font-bold rounded-xl transition-all shadow-lg shadow-red-500/20"
          >
            Seite neu laden
          </button>
        </div>
      );
    }

    return this.props.children;
  }
}
