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
                const wallet = store.playerAccount.wallet;
                useAuthStore.setState({
                    isFaceitConnected: false,
                    isFullyConnected: false,
                    playerAccount: {
                        faceit: null,
                        wallet,
                        isFullyConnected: false,
                    },
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
            // Don't auto-logout for /auth/me — a 401 there simply means
            // "not logged in yet", not "session expired". Auto-logout would
            // destroy a session that was just created during wallet connect.
            const requestUrl = error.config?.url || '';
            if (!requestUrl.includes('/auth/me')) {
                await useAuthStore.getState().logout();
            }
        }

        return Promise.reject(error);
    },
);

export default apiClient;
