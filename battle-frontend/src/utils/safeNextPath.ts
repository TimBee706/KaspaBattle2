/** Only same-origin paths are accepted as post-login targets (no open redirect). */
export function safeNextPath(next: string | null): string {
    return next && /^\/(?![/\\])[A-Za-z0-9\-._~!$&'()*+,;=:@%/?#]*$/.test(next) ? next : '/free-play';
}
