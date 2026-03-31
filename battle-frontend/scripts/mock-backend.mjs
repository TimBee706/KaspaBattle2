import { createServer } from 'node:http';
import { randomUUID } from 'node:crypto';
import { readFileSync, existsSync } from 'node:fs';
import path from 'node:path';

const port = Number(process.env.MOCK_BACKEND_PORT || 8080);
const frontendUrl = process.env.MOCK_FRONTEND_URL || 'http://localhost:5173';
const sessionCookieName = 'kaspabattle-auth';

const challengeStore = new Map();
const sessionStore = new Map();
const lobbyStore = new Map();

const readDotEnvFile = (filePath) => {
  if (!existsSync(filePath)) return {};

  const content = readFileSync(filePath, 'utf8');
  const values = {};
  for (const rawLine of content.split(/\r?\n/)) {
    const line = rawLine.trim();
    if (!line || line.startsWith('#')) continue;

    const separatorIndex = line.indexOf('=');
    if (separatorIndex === -1) continue;

    const key = line.slice(0, separatorIndex).trim();
    let value = line.slice(separatorIndex + 1).trim();
    if (
      (value.startsWith('"') && value.endsWith('"')) ||
      (value.startsWith("'") && value.endsWith("'"))
    ) {
      value = value.slice(1, -1);
    }

    values[key] = value;
  }
  return values;
};

const backendEnvPaths = [
  path.resolve(process.cwd(), '..', 'kaspabattle', '.env.local'),
  path.resolve(process.cwd(), '..', 'kaspabattle', '.env'),
];

const envFiles = backendEnvPaths.map((envPath) => readDotEnvFile(envPath));
for (const envValues of envFiles) {
  for (const [key, value] of Object.entries(envValues)) {
    if (!(key in process.env)) {
      process.env[key] = value;
    }
  }
}

const isPublicUrl = (value) => /^https?:\/\//i.test(value) && !/localhost|127\.0\.0\.1/i.test(value);

const configuredRedirectCandidates = [
  process.env.FACEIT_REDIRECT_URI,
  ...envFiles.map((envValues) => envValues.FACEIT_REDIRECT_URI),
].filter(Boolean);

const redirectUri =
  configuredRedirectCandidates.find((value) => isPublicUrl(value)) ||
  configuredRedirectCandidates[0] ||
  'http://localhost:8080/api/v1/faceit/callback';

const faceitConfig = {
  clientId: process.env.FACEIT_CLIENT_ID || '',
  clientSecret: process.env.FACEIT_CLIENT_SECRET || '',
  redirectUri,
  authUrl: 'https://accounts.faceit.com',
  tokenUrl: 'https://api.faceit.com/auth/v1/oauth/token',
  userInfoUrl: 'https://api.faceit.com/auth/v1/resources/userinfo',
  dataApiKey: process.env.FACEIT_DATA_API_KEY || '',
};

const defaultHeaders = {
  'Access-Control-Allow-Origin': frontendUrl,
  'Access-Control-Allow-Credentials': 'true',
  'Access-Control-Allow-Headers': 'Content-Type, Authorization',
  'Access-Control-Allow-Methods': 'GET, POST, PUT, PATCH, DELETE, OPTIONS',
};

const buildUserProfile = (session) => ({
  id: session.user.id,
  faceit_id: session.user.faceit_id || '',
  faceit_nickname: session.user.faceit_nickname || '',
  faceit_connected: Boolean(session.user.faceit_connected),
  display_name: session.user.display_name || 'Player',
  faceit_avatar: session.user.faceit_avatar || '',
  faceit_elo: session.user.faceit_elo ?? null,
  faceit_skill_level: session.user.faceit_skill_level ?? null,
  kaspa_address: session.user.kaspa_address ?? null,
  total_matches: session.user.total_matches ?? 0,
  wins: session.user.wins ?? 0,
  losses: session.user.losses ?? 0,
  total_wagered_sompi: session.user.total_wagered_sompi ?? 0,
  total_won_sompi: session.user.total_won_sompi ?? 0,
  created_at: session.user.created_at,
});

const parseCookies = (req) => {
  const cookieHeader = req.headers.cookie || '';
  return Object.fromEntries(
    cookieHeader
      .split(';')
      .map((cookie) => cookie.trim())
      .filter(Boolean)
      .map((cookie) => {
        const separatorIndex = cookie.indexOf('=');
        if (separatorIndex === -1) return [cookie, ''];
        return [cookie.slice(0, separatorIndex), decodeURIComponent(cookie.slice(separatorIndex + 1))];
      }),
  );
};

const getSession = (req) => {
  const cookies = parseCookies(req);
  const sessionId = cookies[sessionCookieName];
  return sessionId ? sessionStore.get(sessionId) || null : null;
};

const setSessionCookie = (res, sessionId) => {
  res.setHeader('Set-Cookie', `${sessionCookieName}=${sessionId}; HttpOnly; Path=/; SameSite=Lax; Max-Age=604800`);
};

const clearSessionCookie = (res) => {
  res.setHeader('Set-Cookie', `${sessionCookieName}=; HttpOnly; Path=/; SameSite=Lax; Max-Age=0`);
};

const sendJson = (res, statusCode, body, extraHeaders = {}) => {
  res.writeHead(statusCode, {
    ...defaultHeaders,
    'Content-Type': 'application/json; charset=utf-8',
    ...extraHeaders,
  });
  res.end(JSON.stringify(body));
};

const sendRedirect = (res, location, extraHeaders = {}) => {
  res.writeHead(302, {
    ...defaultHeaders,
    Location: location,
    ...extraHeaders,
  });
  res.end();
};

const sendText = (res, statusCode, body) => {
  res.writeHead(statusCode, {
    ...defaultHeaders,
    'Content-Type': 'text/plain; charset=utf-8',
  });
  res.end(body);
};

const readBody = (req) =>
  new Promise((resolve, reject) => {
    let data = '';
    req.on('data', (chunk) => {
      data += chunk;
    });
    req.on('end', () => resolve(data));
    req.on('error', reject);
  });

const readJsonBody = async (req) => {
  const raw = await readBody(req);
  return raw ? JSON.parse(raw) : {};
};

const getDefaultStats = (nickname) => ({
  game_id: 'cs2',
  lifetime: {
    Matches: '0',
    Wins: '0',
    'Win Rate %': '0',
    'Average K/D Ratio': '0.00',
    'Average Headshots %': '0',
    'Recent Results': [],
    nickname,
  },
  is_cached: true,
});

const fetchFaceitUserInfo = async (code) => {
  if (!faceitConfig.clientId || !faceitConfig.clientSecret) {
    throw new Error('FACEIT client credentials are not configured for mock backend.');
  }

  const tokenResponse = await fetch(faceitConfig.tokenUrl, {
    method: 'POST',
    headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
    body: new URLSearchParams({
      grant_type: 'authorization_code',
      code,
      client_id: faceitConfig.clientId,
      client_secret: faceitConfig.clientSecret,
      redirect_uri: faceitConfig.redirectUri,
    }),
  });

  if (!tokenResponse.ok) {
    const detail = await tokenResponse.text();
    throw new Error(`FACEIT token exchange failed: ${detail}`);
  }

  const tokenPayload = await tokenResponse.json();
  const accessToken = tokenPayload.access_token;
  if (!accessToken) {
    throw new Error('FACEIT token exchange returned no access_token.');
  }

  const userResponse = await fetch(faceitConfig.userInfoUrl, {
    headers: {
      Authorization: `Bearer ${accessToken}`,
      Accept: 'application/json',
    },
  });

  if (!userResponse.ok) {
    const detail = await userResponse.text();
    throw new Error(`FACEIT userinfo request failed: ${detail}`);
  }

  const userInfo = await userResponse.json();
  return { tokenPayload, userInfo };
};

const fetchFaceitStats = async (playerId, gameId) => {
  if (!faceitConfig.dataApiKey || !playerId) {
    return getDefaultStats('');
  }

  const statsResponse = await fetch(
    `https://open.faceit.com/data/v4/players/${encodeURIComponent(playerId)}/stats/${encodeURIComponent(gameId)}`,
    {
      headers: {
        Authorization: `Bearer ${faceitConfig.dataApiKey}`,
        Accept: 'application/json',
      },
    },
  );

  if (!statsResponse.ok) {
    return getDefaultStats('');
  }

  const statsPayload = await statsResponse.json();
  return {
    game_id: gameId,
    lifetime: statsPayload,
    is_cached: false,
  };
};

const createBaseUser = () => ({
  id: randomUUID(),
  faceit_id: '',
  faceit_nickname: '',
  faceit_connected: false,
  display_name: 'Player',
  faceit_avatar: '',
  faceit_elo: null,
  faceit_skill_level: null,
  kaspa_address: null,
  total_matches: 0,
  wins: 0,
  losses: 0,
  total_wagered_sompi: 0,
  total_won_sompi: 0,
  created_at: new Date().toISOString(),
});

const buildMockMatch = ({ session, payload }) => {
  const id = randomUUID();
  const createdAt = new Date().toISOString();
  const wagerSompi = Number(payload.wager_sompi ?? payload.stake_kas ?? 0);
  const matchMode = String(payload.mode || 'BO1').toUpperCase();
  const gameId = String(payload.game_id || 'CS2').toUpperCase();
  const escrowAddress = payload.escrow_address || session.user.kaspa_address || 'kaspatest:mock-escrow';

  return {
    id,
    creator_user_id: session.user.id,
    opponent_user_id: null,
    player_a_kas_address: session.user.kaspa_address,
    player_b_kas_address: null,
    player_a_faceit_id: session.user.faceit_id || '',
    player_b_faceit_id: null,
    player_a_faceit_nickname: session.user.faceit_nickname || session.user.display_name || 'Player A',
    player_b_faceit_nickname: null,
    faceit_match_id: null,
    wager_amount_sompi: wagerSompi,
    stake_kas: wagerSompi,
    escrow_address: escrowAddress,
    status: 'AWAITING_FUNDING',
    game_id: gameId,
    match_mode: matchMode,
    mode: matchMode,
    winner_kas_address: null,
    winner_faceit_nickname: null,
    payout_tx_hash: null,
    score: null,
    player_a_deposit_tx_hash: null,
    player_b_deposit_tx_hash: null,
    created_at: createdAt,
    locked_at: null,
    resolved_at: null,
    timeout_at: new Date(Date.now() + 90 * 60_000).toISOString(),
  };
};

const buildPaymentInfo = (txHash, amountSompi) => ({
  paid: Boolean(txHash),
  confirmed_sompi: txHash ? amountSompi : 0,
  payment_count: txHash ? 1 : 0,
  min_confirmations: txHash ? 10 : 0,
});

const buildPaymentStatus = (match) => {
  const required = match.wager_amount_sompi || match.stake_kas || 0;
  const playerA = buildPaymentInfo(match.player_a_deposit_tx_hash, required);
  const playerB = buildPaymentInfo(match.player_b_deposit_tx_hash, required);
  return {
    escrow_address: match.escrow_address,
    required_per_player_sompi: required,
    min_confirmations_required: 10,
    playerA,
    playerB,
    both_paid: playerA.paid && playerB.paid,
  };
};

const ensureSession = (req) => {
  const existing = getSession(req);
  if (existing) return existing;

  const sessionId = randomUUID();
  const session = { id: sessionId, user: createBaseUser() };
  sessionStore.set(sessionId, session);
  return session;
};

const server = createServer(async (req, res) => {
  const url = new URL(req.url || '/', `http://${req.headers.host || `localhost:${port}`}`);
  const { pathname } = url;

  if (req.method === 'OPTIONS') {
    res.writeHead(204, defaultHeaders);
    res.end();
    return;
  }

  if (pathname === '/') {
    sendJson(res, 200, {
      ok: true,
      service: 'KaspaBattle local mock backend',
      api_base: '/api/v1',
      websocket: '/ws',
      mode: 'dev-fallback',
    });
    return;
  }

  if (pathname === '/ws') {
    sendText(res, 426, 'Mock backend does not provide WebSocket support.');
    return;
  }

  if (pathname === '/api/v1/lobbies' && req.method === 'GET') {
    sendJson(res, 200, Array.from(lobbyStore.values()));
    return;
  }

  if (pathname === '/api/v1/history' && req.method === 'GET') {
    sendJson(res, 200, []);
    return;
  }

  if (pathname === '/api/v1/challenges' && req.method === 'POST') {
    const session = getSession(req);
    if (!session || !session.user.kaspa_address) {
      sendJson(res, 401, {
        error: 'wallet_required',
        message: 'Connect a wallet first before creating a challenge.',
      });
      return;
    }

    const payload = await readJsonBody(req);
    const match = buildMockMatch({ session, payload });
    lobbyStore.set(match.id, match);
    sendJson(res, 200, match);
    return;
  }

  if (pathname === '/api/v1/auth/me' && req.method === 'GET') {
    const session = getSession(req);
    if (!session) {
      sendJson(res, 401, {
        error: 'not_authenticated',
        message: 'No active local session.',
      });
      return;
    }

    sendJson(res, 200, buildUserProfile(session));
    return;
  }

  if (pathname === '/api/v1/auth/logout' && req.method === 'POST') {
    const session = getSession(req);
    if (session) {
      sessionStore.delete(session.id);
    }
    clearSessionCookie(res);
    sendJson(res, 200, { success: true }, { 'Set-Cookie': `${sessionCookieName}=; HttpOnly; Path=/; SameSite=Lax; Max-Age=0` });
    return;
  }

  if (pathname === '/api/v1/auth/wallet-challenge' && req.method === 'POST') {
    const payload = await readJsonBody(req);
    const challengeId = randomUUID();
    const expiresAt = new Date(Date.now() + 5 * 60_000).toISOString();
    const challenge = {
      challenge_id: challengeId,
      kaspa_address: payload.kaspa_address || 'kaspatest:mock-address',
      message: `KaspaBattle Login Challenge\nAddress: ${payload.kaspa_address || 'kaspatest:mock-address'}\nNonce: ${randomUUID()}\nChallenge ID: ${challengeId}\nExpires At: ${expiresAt}`,
      expires_at: expiresAt,
    };

    challengeStore.set(challengeId, challenge);
    sendJson(res, 200, challenge);
    return;
  }

  if (pathname === '/api/v1/auth/wallet-verify' && req.method === 'POST') {
    const payload = await readJsonBody(req);
    const challenge = challengeStore.get(payload.challenge_id);

    if (!challenge || challenge.kaspa_address !== payload.kaspa_address) {
      sendJson(res, 401, {
        error: 'invalid_challenge',
        message: 'Wallet challenge is missing or does not match the wallet address.',
      });
      return;
    }

    const session = ensureSession(req);
    session.user.kaspa_address = payload.kaspa_address;
    session.user.display_name = session.user.faceit_nickname || `Player_${String(payload.kaspa_address).slice(6, 12)}`;

    setSessionCookie(res, session.id);
    sendJson(
      res,
      200,
      {
        user_id: session.user.id,
        display_name: session.user.display_name,
      },
      { 'Set-Cookie': `${sessionCookieName}=${session.id}; HttpOnly; Path=/; SameSite=Lax; Max-Age=604800` },
    );
    return;
  }

  if (pathname === '/api/v1/faceit/login' && req.method === 'GET') {
    if (!faceitConfig.clientId) {
      sendJson(res, 503, {
        error: 'faceit_not_configured',
        message: 'FACEIT_CLIENT_ID is not configured for the local mock backend.',
      });
      return;
    }

    const state = randomUUID();
    const authorizeUrl = new URL(faceitConfig.authUrl);
    authorizeUrl.searchParams.set('client_id', faceitConfig.clientId);
    authorizeUrl.searchParams.set('redirect_uri', faceitConfig.redirectUri);
    authorizeUrl.searchParams.set('response_type', 'code');
    authorizeUrl.searchParams.set('scope', 'openid profile email');
    authorizeUrl.searchParams.set('state', state);

    sendRedirect(res, authorizeUrl.toString());
    return;
  }

  if (pathname === '/api/v1/faceit/callback' && req.method === 'GET') {
    const code = url.searchParams.get('code');
    const state = url.searchParams.get('state');
    if (!code || !state) {
      sendRedirect(res, `${frontendUrl}/?error=missing_faceit_callback_params`);
      return;
    }

    try {
      const { userInfo } = await fetchFaceitUserInfo(code);
      const session = ensureSession(req);

      const games = userInfo.games || {};
      const cs2 = games.cs2 || games.csgo || {};

      session.user.faceit_id = userInfo.guid || userInfo.player_id || '';
      session.user.faceit_nickname = userInfo.nickname || userInfo.nick || 'FACEIT User';
      session.user.faceit_connected = true;
      session.user.faceit_avatar = userInfo.picture || userInfo.avatar || '';
      session.user.faceit_elo = typeof cs2.faceit_elo === 'number' ? cs2.faceit_elo : null;
      session.user.faceit_skill_level = typeof cs2.skill_level === 'number' ? cs2.skill_level : null;
      session.user.display_name = session.user.faceit_nickname || session.user.display_name;

      sendRedirect(
        res,
        `${frontendUrl}/?linked=1`,
        { 'Set-Cookie': `${sessionCookieName}=${session.id}; HttpOnly; Path=/; SameSite=Lax; Max-Age=604800` },
      );
    } catch (error) {
      const message = error instanceof Error ? error.message : 'faceit_callback_failed';
      sendRedirect(res, `${frontendUrl}/?error=${encodeURIComponent(message)}`);
    }
    return;
  }

  if (pathname === '/api/v1/faceit/status' && req.method === 'GET') {
    const session = getSession(req);
    if (!session) {
      sendJson(res, 401, { error: 'not_authenticated' });
      return;
    }

    sendJson(res, 200, {
      connected: Boolean(session.user.faceit_connected),
      faceit_nickname: session.user.faceit_nickname || null,
      faceit_avatar_url: session.user.faceit_avatar || null,
      faceit_elo: session.user.faceit_elo,
      faceit_skill_level: session.user.faceit_skill_level,
      linked_at: session.user.faceit_connected ? session.user.created_at : null,
    });
    return;
  }

  const acceptMatch = pathname.match(/^\/api\/v1\/matches\/([^/]+)\/accept$/);
  if (acceptMatch && req.method === 'POST') {
    const session = getSession(req);
    if (!session || !session.user.kaspa_address) {
      sendJson(res, 401, {
        error: 'wallet_required',
        message: 'Connect a wallet first before joining a challenge.',
      });
      return;
    }

    const match = lobbyStore.get(acceptMatch[1]);
    if (!match) {
      sendJson(res, 404, { error: 'match_not_found' });
      return;
    }

    match.opponent_user_id = session.user.id;
    match.player_b_kas_address = session.user.kaspa_address;
    match.player_b_faceit_id = session.user.faceit_id || null;
    match.player_b_faceit_nickname = session.user.faceit_nickname || session.user.display_name || 'Player B';
    match.status = 'AWAITING_FUNDING';
    lobbyStore.set(match.id, match);
    sendJson(res, 200, match);
    return;
  }

  const submitDeposit = pathname.match(/^\/api\/v1\/matches\/([^/]+)\/deposit$/);
  if (submitDeposit && req.method === 'POST') {
    const session = getSession(req);
    if (!session) {
      sendJson(res, 401, { error: 'not_authenticated' });
      return;
    }

    const match = lobbyStore.get(submitDeposit[1]);
    if (!match) {
      sendJson(res, 404, { error: 'match_not_found' });
      return;
    }

    const payload = await readJsonBody(req);
    if (payload.player_role === 'A') {
      match.player_a_deposit_tx_hash = payload.tx_hash || null;
    } else if (payload.player_role === 'B') {
      match.player_b_deposit_tx_hash = payload.tx_hash || null;
    }

    const paymentStatus = buildPaymentStatus(match);
    if (paymentStatus.both_paid) {
      match.status = 'FUNDED';
      match.locked_at = new Date().toISOString();
    }

    lobbyStore.set(match.id, match);
    sendJson(res, 200, match);
    return;
  }

  const paymentStatusRoute = pathname.match(/^\/api\/v1\/matches\/([^/]+)\/payment-status$/);
  if (paymentStatusRoute && req.method === 'GET') {
    const match = lobbyStore.get(paymentStatusRoute[1]);
    if (!match) {
      sendJson(res, 404, { error: 'match_not_found' });
      return;
    }

    sendJson(res, 200, buildPaymentStatus(match));
    return;
  }

  if (pathname === '/api/v1/faceit/profile' && req.method === 'GET') {
    const session = getSession(req);
    if (!session || !session.user.faceit_connected) {
      sendJson(res, 404, { error: 'faceit_not_connected' });
      return;
    }

    sendJson(res, 200, {
      faceit_player_id: session.user.faceit_id,
      nickname: session.user.faceit_nickname,
      avatar_url: session.user.faceit_avatar || null,
      country: '',
      elo: session.user.faceit_elo ?? 0,
      skill_level: session.user.faceit_skill_level ?? 0,
      games: ['cs2'],
      faceit_url: `https://www.faceit.com/en/players/${encodeURIComponent(session.user.faceit_nickname)}`,
      is_cached: true,
    });
    return;
  }

  if (pathname === '/api/v1/faceit/stats' && req.method === 'GET') {
    const session = getSession(req);
    if (!session || !session.user.faceit_connected) {
      sendJson(res, 404, { error: 'faceit_not_connected' });
      return;
    }

    const game = url.searchParams.get('game') || 'cs2';
    const stats = await fetchFaceitStats(session.user.faceit_id, game);
    if (stats.is_cached) {
      stats.lifetime.nickname = session.user.faceit_nickname;
    }
    sendJson(res, 200, stats);
    return;
  }

  if (pathname === '/api/v1/faceit/disconnect' && req.method === 'POST') {
    const session = getSession(req);
    if (!session) {
      sendJson(res, 401, { error: 'not_authenticated' });
      return;
    }

    session.user.faceit_id = '';
    session.user.faceit_nickname = '';
    session.user.faceit_connected = false;
    session.user.faceit_avatar = '';
    session.user.faceit_elo = null;
    session.user.faceit_skill_level = null;

    sendJson(res, 200, { success: true });
    return;
  }

  if (pathname.startsWith('/api/v1/')) {
    sendJson(res, 501, {
      error: 'mock_not_implemented',
      path: pathname,
      message: 'This endpoint is not implemented in the local mock backend.',
    });
    return;
  }

  sendJson(res, 404, {
    error: 'not_found',
    path: pathname,
  });
});

server.listen(port, '127.0.0.1', () => {
  console.log(`[mock-backend] listening on http://127.0.0.1:${port}`);
  console.log(`[mock-backend] frontend: ${frontendUrl}`);
  console.log(`[mock-backend] faceit redirect URI: ${faceitConfig.redirectUri}`);
});

const shutdown = () => {
  server.close(() => process.exit(0));
};

process.on('SIGINT', shutdown);
process.on('SIGTERM', shutdown);
