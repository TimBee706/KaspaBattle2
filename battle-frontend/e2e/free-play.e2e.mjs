// Free Play E2E: run against a real backend (+ Postgres) and the Vite dev server.
// Usage: NODE_PATH=$(npm root -g) E2E_BASE_URL=http://localhost:5173 node e2e/free-play.e2e.mjs
// No wallet, FACEIT or Kaspa node is needed (or touched) by any step.
import { createRequire } from 'node:module';
import assert from 'node:assert/strict';
const require = createRequire(import.meta.url);
const { chromium } = require('playwright');

const BASE = process.env.E2E_BASE_URL ?? 'http://localhost:5173';
const run = Date.now().toString(36);
const PASSWORD = `correct-horse-${run}-battery`;

const browser = await chromium.launch();
const apiCalls = [];
const errors = [];

async function newUser(name) {
    const ctx = await browser.newContext({ locale: 'de-DE' });
    const page = await ctx.newPage();
    page.on('request', (r) => { if (r.url().includes('/api/')) apiCalls.push(new URL(r.url()).pathname); });
    page.on('pageerror', (e) => errors.push(`${name}: ${e.message}`));
    return { ctx, page, name, email: `${name}-${run}@example.test` };
}

async function register(u) {
    const { page } = u;
    await page.goto(`${BASE}/register`);
    await page.fill('#reg-username', u.name);
    await page.fill('#reg-email', u.email);
    await page.fill('#reg-password', PASSWORD);
    await page.fill('#reg-password2', PASSWORD);
    await page.locator('input[type=checkbox]').first().check();
    await page.getByRole('button', { name: 'Konto erstellen' }).click();
    await page.waitForURL(/\/register/);
    await page.getByRole('link', { name: /Anmelden/ }).first().click();
}

async function login(u) {
    const { page } = u;
    await page.goto(`${BASE}/login?next=/free-play`);
    await page.fill('#login-email', u.email);
    await page.fill('#login-password', PASSWORD);
    await page.getByRole('button', { name: 'Anmelden' }).click();
    await page.waitForURL(/\/free-play$/);
    await page.getByRole('heading', { name: 'Kostenlose Lobby erstellen' }).or(page.getByText('Kostenlose Lobby erstellen').first()).waitFor();
}

async function drop(page, column) {
    const btn = page.locator(`button[data-column="${column}"][aria-disabled="false"]`);
    await btn.waitFor({ timeout: 15000 });
    await btn.click();
}

try {
    // 1. landing notice
    const anon = await newUser('anon');
    await anon.page.goto(BASE);
    const notice = anon.page.getByTestId('node-help-notice');
    await notice.waitFor();
    assert.match(await notice.innerText(), /öffentlichen Beta/);
    assert.equal(await notice.locator('a[href^="mailto:"]').getAttribute('href'), 'mailto:info@kaspabattle.com');
    await anon.ctx.close();

    // 2. two registered players, PvP
    const a = await newUser(`alice${run}`.slice(0, 20));
    const b = await newUser(`bob${run}`.slice(0, 20));
    await register(a); await register(b);
    await login(a); await login(b);

    // reload keeps the session
    await a.page.reload();
    await a.page.getByText('Kostenlose Lobby erstellen').first().waitFor();

    await a.page.getByRole('button', { name: 'Kostenlose Lobby erstellen' }).click();
    await a.page.waitForURL(/\/free-play\/[0-9a-f-]+$/);
    await b.page.getByRole('button', { name: 'Spiel beitreten' }).first().click();
    await b.page.waitForURL(/\/free-play\/[0-9a-f-]+$/);
    await a.page.getByTestId('free-play-game').waitFor();

    // creator is blue/p1 or the server may randomise: play until someone wins with vertical stacks
    const pages = [a.page, b.page];
    const columnFor = new Map([[a.page, 0], [b.page, 1]]);
    let winner = null;
    for (let i = 0; i < 30 && !winner; i++) {
        for (const p of pages) {
            const enabled = p.locator('button[data-column][aria-disabled="false"]');
            if (await enabled.count()) {
                await enabled.nth(columnFor.get(p) % await enabled.count()).click();
                await p.waitForTimeout(150);
            }
        }
        for (const p of pages) {
            if (await p.getByText('Du hast gewonnen!').count()) winner = p;
        }
    }
    assert.ok(winner, 'one player must win the PvP game');
    const loser = pages.find((p) => p !== winner);
    await loser.getByText('Du hast verloren').waitFor();

    // reload of finished game shows the result (server state, not client memory)
    await loser.reload();
    await loser.getByText('Du hast verloren').waitFor();

    // 3. bot game
    await a.page.goto(`${BASE}/free-play`);
    await a.page.getByRole('button', { name: 'Bot-Spiel starten' }).click();
    await a.page.waitForURL(/\/free-play\/[0-9a-f-]+$/);
    for (let i = 0; i < 3; i++) {
        await drop(a.page, i % 7);
        await a.page.waitForTimeout(300);
    }
    await a.page.locator('button[data-column][aria-disabled="false"]').first().waitFor({ timeout: 15000 });

    // 4. nothing wallet/payment related was touched
    const forbidden = apiCalls.filter((p) => /wallet|escrow|deposit|payout|faceit|rpc/i.test(p));
    assert.deepEqual(forbidden, [], `free play must not call ${forbidden.join(',')}`);
    assert.deepEqual(errors, [], 'no uncaught page errors');
    console.log('E2E OK: register, login, session reload, PvP win/loss, bot game, landing notice');
} finally {
    await browser.close();
}
