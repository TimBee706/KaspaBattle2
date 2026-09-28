export class KaspaRpcError extends Error {
    code: string;
    technicalDetail?: string;

    constructor(code: string, message: string, options?: { technicalDetail?: string }) {
        super(message);
        this.name = 'KaspaRpcError';
        this.code = code;
        this.technicalDetail = options?.technicalDetail;
    }
}
