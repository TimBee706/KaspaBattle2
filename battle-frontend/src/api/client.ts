import axios from 'axios';
import type { AxiosInstance } from 'axios';
import { API_BASE_URL } from '../config/constants';
import { useAuthStore } from '../stores/useAuthStore';

const apiClient: AxiosInstance = axios.create({
    baseURL: API_BASE_URL,
    timeout: 15_000,
    withCredentials: true,
    headers: { 'Content-Type': 'application/json' },
});

apiClient.interceptors.response.use(
    (response) => response,
    async (error) => {
        const status = error.response?.status;
        const errorCode = error.response?.data?.error;

        if (status === 401 && errorCode === 'faceit_reauth_required') {
            const store = useAuthStore.getState();
            if (store.user) {
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
            window.dispatchEvent(new CustomEvent('faceit-reauth-required'));
            return Promise.reject(error);
        }

        if (status === 401) {
            await useAuthStore.getState().logout();
        }

        return Promise.reject(error);
    },
);

export default apiClient;
