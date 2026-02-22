import axios from 'axios';
import type { AxiosInstance, InternalAxiosRequestConfig } from 'axios';
import { API_BASE_URL } from '../config/constants';
import { useAuthStore } from '../stores/useAuthStore';

const apiClient: AxiosInstance = axios.create({
    baseURL: API_BASE_URL,
    timeout: 15_000,
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
apiClient.interceptors.response.use(
    (response) => response,
    async (error) => {
        if (error.response?.status === 401) {
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
