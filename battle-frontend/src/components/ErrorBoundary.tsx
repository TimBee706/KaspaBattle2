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
          <svg xmlns="http://www.w3.org/2000/svg" className="w-14 h-14 text-red-500 mb-6" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.75}>
            <path strokeLinecap="round" strokeLinejoin="round" d="M12 9v4m0 4h.01M10.29 3.86L1.82 18a2 2 0 001.71 3h16.94a2 2 0 001.71-3L13.71 3.86a2 2 0 00-3.42 0z" />
          </svg>
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
