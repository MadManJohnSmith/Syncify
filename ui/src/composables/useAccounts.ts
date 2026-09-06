/**
 * Accounts Composable
 * 
 * State management for services and accounts.
 * 
 * @deprecated Zombie composable. Use useAccountsStatus.ts instead. Do not import in production views.
 */

import { ref, computed } from 'vue';
import { accountsApi } from '@/api/accounts';
import type {
    Service,
    Account,
    ServiceStatus,
    SessionStatus,
    AuthResult,
    ImportResult
} from '@/api/types';

/**
 * Composable for accounts state management
 * 
 * @deprecated Zombie composable. Use useAccountsStatus.ts instead. Do not import in production views.
 */
export function useAccounts() {
    // State
    const services = ref<Service[]>([]);
    const accounts = ref<Account[]>([]);
    const serviceStatuses = ref<ServiceStatus[]>([]);
    const sessionStatuses = ref<SessionStatus[]>([]);
    const loading = ref(false);
    const authLoading = ref(false);
    const importLoading = ref(false);
    const error = ref<Error | null>(null);

    // Computed
    const connectedAccounts = computed(() =>
        accounts.value.filter(a => a.is_active)
    );

    const disconnectedServices = computed(() => {
        const connectedServiceIds = new Set(accounts.value.map(a => a.service_id));
        return services.value.filter(s => !connectedServiceIds.has(s.id));
    });

    const hasConnectedAccounts = computed(() =>
        connectedAccounts.value.length > 0
    );

    // Actions
    async function fetchServices(): Promise<void> {
        loading.value = true;
        error.value = null;

        try {
            services.value = await accountsApi.getServices();
        } catch (e) {
            error.value = e instanceof Error ? e : new Error(String(e));
        } finally {
            loading.value = false;
        }
    }

    async function fetchAccounts(): Promise<void> {
        loading.value = true;
        error.value = null;

        try {
            accounts.value = await accountsApi.getAccounts();
        } catch (e) {
            error.value = e instanceof Error ? e : new Error(String(e));
        } finally {
            loading.value = false;
        }
    }

    async function fetchServiceStatuses(): Promise<void> {
        try {
            serviceStatuses.value = await accountsApi.getServiceStatuses();
        } catch (e) {
            console.error('Failed to fetch service statuses:', e);
        }
    }

    async function validateSessions(): Promise<SessionStatus[]> {
        try {
            sessionStatuses.value = await accountsApi.validateAllSessions();
            return sessionStatuses.value;
        } catch (e) {
            console.error('Failed to validate sessions:', e);
            return [];
        }
    }

    // Authentication
    async function connectService(serviceName: string): Promise<AuthResult> {
        authLoading.value = true;

        try {
            const result = await accountsApi.startAuthAndSave(serviceName);

            if (result.success) {
                // Refresh accounts after successful connection
                await fetchAccounts();
                await fetchServiceStatuses();
            }

            return result;
        } finally {
            authLoading.value = false;
        }
    }

    async function disconnectService(serviceName: string): Promise<void> {
        authLoading.value = true;

        try {
            await accountsApi.logoutService(serviceName);

            // Find and remove account
            const account = accounts.value.find(
                a => services.value.find(s => s.id === a.service_id)?.name === serviceName
            );

            if (account) {
                await accountsApi.removeAccount(account.id);
            }

            // Refresh accounts
            await fetchAccounts();
            await fetchServiceStatuses();
        } finally {
            authLoading.value = false;
        }
    }

    async function toggleAccountActive(
        accountId: number,
        isActive: boolean
    ): Promise<void> {
        await accountsApi.toggleAccountActive(accountId, isActive);
        await fetchAccounts();
    }

    async function removeAccount(accountId: number): Promise<void> {
        await accountsApi.removeAccount(accountId);
        await fetchAccounts();
        await fetchServiceStatuses();
    }

    // Import
    async function importFromService(serviceName: string): Promise<ImportResult | null> {
        importLoading.value = true;

        try {
            let result: ImportResult;

            switch (serviceName.toLowerCase()) {
                case 'spotify':
                    result = await accountsApi.importSpotifyLibrary();
                    break;
                case 'qobuz':
                    result = await accountsApi.importQobuzLibrary();
                    break;
                case 'tidal':
                    result = await accountsApi.importTidalLibrary();
                    break;
                case 'deezer':
                    result = await accountsApi.importDeezerLibrary();
                    break;
                default:
                    throw new Error(`Unknown service: ${serviceName}`);
            }

            // Update sync time
            const account = accounts.value.find(
                a => services.value.find(s => s.id === a.service_id)?.name === serviceName
            );

            if (account) {
                await accountsApi.updateAccountSyncTime(account.id);
                await fetchAccounts();
            }

            return result;
        } catch (e) {
            error.value = e instanceof Error ? e : new Error(String(e));
            return null;
        } finally {
            importLoading.value = false;
        }
    }

    // Helper to get service by name
    function getServiceByName(name: string): Service | undefined {
        return services.value.find(s => s.name.toLowerCase() === name.toLowerCase());
    }

    // Helper to get account for service
    function getAccountForService(serviceName: string): Account | undefined {
        const service = getServiceByName(serviceName);
        if (!service) return undefined;
        return accounts.value.find(a => a.service_id === service.id);
    }

    // Initialize
    async function initialize(): Promise<void> {
        await Promise.all([
            fetchServices(),
            fetchAccounts(),
            fetchServiceStatuses(),
        ]);
    }

    return {
        // State
        services,
        accounts,
        serviceStatuses,
        sessionStatuses,
        loading,
        authLoading,
        importLoading,
        error,

        // Computed
        connectedAccounts,
        disconnectedServices,
        hasConnectedAccounts,

        // Actions
        fetchServices,
        fetchAccounts,
        fetchServiceStatuses,
        validateSessions,
        connectService,
        disconnectService,
        toggleAccountActive,
        removeAccount,
        importFromService,

        // Helpers
        getServiceByName,
        getAccountForService,
        initialize,
    };
}
