type UnknownRecord = Record<string, unknown>;

function isRecord(value: unknown): value is UnknownRecord {
    return typeof value === 'object' && value !== null;
}

export function getErrorMessage(error: unknown, fallback = 'Unexpected error'): string {
    if (error instanceof Error && error.message) {
        return error.message;
    }

    if (isRecord(error) && typeof error.message === 'string') {
        return error.message;
    }

    if (typeof error === 'string') {
        return error;
    }

    return fallback;
}

export function getApiErrorCode(error: unknown): string | undefined {
    if (!isRecord(error) || !isRecord(error.response) || !isRecord(error.response.data)) {
        return undefined;
    }

    return typeof error.response.data.error === 'string' ? error.response.data.error : undefined;
}

export function getApiErrorMessage(error: unknown): string | undefined {
    if (!isRecord(error) || !isRecord(error.response) || !isRecord(error.response.data)) {
        return undefined;
    }

    return typeof error.response.data.message === 'string' ? error.response.data.message : undefined;
}
