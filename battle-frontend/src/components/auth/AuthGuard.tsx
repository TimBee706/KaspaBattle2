import { Navigate, useLocation } from 'react-router-dom';
import { useAuthStore } from '../../stores/useAuthStore';

export function AuthGuard({ children }: { children: React.ReactNode }) {
    const { isAuthenticated, isAuthLoading, testMode } = useAuthStore();
    const location = useLocation();

    // Wait for initial auth check to complete before deciding
    if (isAuthLoading && !testMode) {
        return (
            <div className="flex items-center justify-center min-h-[60vh]">
                <div className="animate-spin w-8 h-8 border-2 border-kaspa-primary border-t-transparent rounded-full" />
            </div>
        );
    }

    if (!testMode && !isAuthenticated) {
        return <Navigate to="/" state={{ from: location }} replace />;
    }

    return <>{children}</>;
}
