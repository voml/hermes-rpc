import type { GithubAuthor } from "./types.js";

const USER_AGENT = "nifty-github";
const ACCEPT = "application/vnd.github+json";

function parseUserJson(body: Record<string, unknown>, fallbackLogin?: string): GithubAuthor {
    const id = typeof body.id === "number" ? BigInt(body.id) : undefined;
    const login = typeof body.login === "string" ? body.login : fallbackLogin;
    if (id === undefined && login === undefined) {
        throw new Error("GitHub user response missing id and login");
    }
    return { id, login };
}

async function getJson(url: string, token?: string): Promise<Record<string, unknown>> {
    const headers: Record<string, string> = {
        Accept: ACCEPT,
        "User-Agent": USER_AGENT,
    };
    if (token?.trim()) {
        headers.Authorization = `Bearer ${token.trim()}`;
    }
    const response = await fetch(url, { headers });
    if (response.status === 404) {
        throw new Error("GitHub resource not found");
    }
    if (!response.ok) {
        throw new Error(`GitHub API error: ${response.status} ${response.statusText}`);
    }
    return (await response.json()) as Record<string, unknown>;
}

function parseNoreplyEmail(email: string): GithubAuthor | undefined {
    const trimmed = email.trim();
    const suffix = "@users.noreply.github.com";
    const lower = trimmed.toLowerCase();
    if (!lower.endsWith(suffix)) {
        return undefined;
    }
    const local = trimmed.slice(0, -suffix.length);
    const plus = local.indexOf("+");
    if (plus >= 0) {
        const idPart = local.slice(0, plus);
        const login = local.slice(plus + 1);
        if (!login) {
            return undefined;
        }
        return { id: BigInt(idPart), login };
    }
    if (!local) {
        return undefined;
    }
    return { login: local };
}

function loadAuthorMap(json: string): Map<string, GithubAuthor> {
    const map = new Map<string, GithubAuthor>();
    if (!json.trim()) {
        return map;
    }
    const parsed = JSON.parse(json) as Record<string, { id?: number; login?: string }>;
    for (const [email, entry] of Object.entries(parsed)) {
        const author: GithubAuthor = {};
        if (typeof entry.id === "number") {
            author.id = BigInt(entry.id);
        }
        if (typeof entry.login === "string" && entry.login.trim()) {
            author.login = entry.login.trim();
        }
        if (author.id !== undefined || author.login !== undefined) {
            map.set(email.trim().toLowerCase(), author);
        }
    }
    return map;
}

function mergeAuthors(existing: GithubAuthor, incoming: GithubAuthor): GithubAuthor {
    return {
        id: existing.id ?? incoming.id,
        login: existing.login ?? incoming.login,
    };
}

async function enrichAuthor(author: GithubAuthor, token?: string, fetch = false): Promise<GithubAuthor> {
    if (!fetch && author.id !== undefined && author.login !== undefined) {
        return author;
    }
    if (!fetch && author.id !== undefined) {
        return author;
    }
    if (author.login && (author.id === undefined || fetch)) {
        const fetched = await userByLoginFetch(author.login, token);
        return mergeAuthors(author, fetched);
    }
    return author;
}

async function userByLoginFetch(login: string, token?: string): Promise<GithubAuthor> {
    const trimmed = login.trim();
    if (!trimmed) {
        throw new Error("login must not be empty");
    }
    const body = await getJson(`https://api.github.com/users/${trimmed}`, token);
    return parseUserJson(body, trimmed);
}

async function searchUserByEmailFetch(email: string, token: string): Promise<GithubAuthor | undefined> {
    const trimmed = email.trim();
    const auth = token.trim();
    if (!trimmed) {
        throw new Error("email must not be empty");
    }
    if (!auth) {
        throw new Error("GitHub token is required for email search");
    }
    const query = encodeURIComponent(`${trimmed} in:email`);
    const body = await getJson(`https://api.github.com/search/users?q=${query}`, auth);
    const items = body.items;
    if (!Array.isArray(items) || items.length === 0) {
        return undefined;
    }
    const first = items[0] as Record<string, unknown>;
    return parseUserJson(first);
}

export type LookupUserOptions = {
    authorMapJson?: string;
    token?: string;
    fetch?: boolean;
};

/**
 * GitHub REST helpers via host `fetch` (Node 18+, browsers with CORS).
 * Rust equivalent: `nifty-github` crate.
 */
export class Github {
    constructor(private readonly token?: string) {}

    static open(token?: string): Github {
        return new Github(token);
    }

    async userByLogin(login: string, token?: string): Promise<GithubAuthor> {
        return userByLoginFetch(login, token ?? this.token);
    }

    async searchUserByEmail(email: string, token?: string): Promise<GithubAuthor | undefined> {
        const auth = token ?? this.token;
        if (!auth) {
            throw new Error("GitHub token is required for email search");
        }
        return searchUserByEmailFetch(email, auth);
    }

    async lookupUserByEmail(email: string, options: LookupUserOptions = {}): Promise<GithubAuthor | undefined> {
        const token = options.token ?? this.token;
        const fetch = options.fetch ?? false;
        const mapJson = options.authorMapJson ?? "{}";

        const fromNoreply = parseNoreplyEmail(email);
        if (fromNoreply) {
            return enrichAuthor(fromNoreply, token, fetch);
        }

        const map = loadAuthorMap(mapJson);
        const mapped = map.get(email.trim().toLowerCase());
        if (mapped) {
            return enrichAuthor(mapped, token, fetch);
        }

        if (token) {
            const found = await searchUserByEmailFetch(email, token);
            if (found) {
                return enrichAuthor(found, token, fetch);
            }
        }

        return undefined;
    }
}

export function createGithub(token?: string): Github {
    return Github.open(token);
}
