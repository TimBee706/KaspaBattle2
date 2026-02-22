import { Navigate, useLocation } from 'react-router-dom';
import { useAuthStore } from '../../stores/useAuthStore';

export function AuthGuard({ children }: { children: React.ReactNode }) {
    const { isAuthenticated } = useAuthStore();
    const location = useLocation();

    if (!isAuthenticated) {
        // Redirect zu "/" aber State speichern für Redirect nach Login (optional)
        return <Navigate to="/" state={{ from: location }} replace />;
    }

    return <>{children}</>;
}
