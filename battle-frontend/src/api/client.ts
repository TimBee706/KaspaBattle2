import axios from 'axios';
import type { AxiosInstance, InternalAxiosRequestConfig } from 'axios';
import { API_BASE_URL } from '../config/constants';
import { useAuthStore } from '../stores/useAuthStore';

const apiClient: AxiosInstance = axios.create({
    baseURL: API_BASE_URL,
    timeout: 15_000,
    withCredentials: true,
    headers: { 'Content-Type': 'application/json' },
});

// Request Interceptor: Auth Token anhängen
apiClient.interceptors.request.use((config: InternalAxiosRequestConfig) => {
    const { tokens } = useAuthStore.getState();
    if (tokens?.access_token) {
        config.headers.Authorization = `Bearer ${tokens.access_token}`;
    }
    return config;
});

// Response Interceptor: 401 → Token Refresh oder Logout
// + faceit_reauth_required Handling
apiClient.interceptors.response.use(
    (response) => response,
    async (error) => {
        const status = error.response?.status;
        const errorCode = error.response?.data?.error;

        // FACEIT token expired beyond refresh → clear FACEIT status, don't logout
        if (status === 401 && errorCode === 'faceit_reauth_required') {
            const store = useAuthStore.getState();
            if (store.user) {
                // Clear FACEIT connection without full logout
                useAuthStore.setState({
                    isFaceitConnected: false,
                    user: {
                        ...store.user,
                        faceit_connected: false,
                        faceit_id: '',
                        faceit_nickname: '',
                        faceit_avatar: '',
                        faceit_elo: null,
                        faceit_skill_level: null,
                    },
                });
            }
            // Dispatch custom event so UI components can show reconnect banner
            window.dispatchEvent(new CustomEvent('faceit-reauth-required'));
            return Promise.reject(error);
        }

        // Regular 401: session expired
        if (status === 401) {
            const { tokens, logout, refreshAccessToken } = useAuthStore.getState();
            if (tokens?.refresh_token) {
                try {
                    await refreshAccessToken();
                    // Retry original request
                    error.config.headers.Authorization =
                        `Bearer ${useAuthStore.getState().tokens?.access_token}`;
                    return apiClient.request(error.config);
                } catch {
                    logout();
                }
            } else {
                logout();
            }
        }
        return Promise.reject(error);
    },
);

export default apiClient;
