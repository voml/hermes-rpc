import type { CommitRecord, GithubAuthor, ParsedSubject, RangeInfo, ReleaseSection, TagInfo } from "./types.js";
import { loadNiftyNative, type GitmojiExports } from "./native.js";

/**
 * Nifty default convention: every commit subject starts with a gitmoji followed by a space.
 */
export class Gitmoji {
    constructor(
        private readonly wasm: GitmojiExports,
        private readonly defaultAuthorMapJson = "{}",
    ) {}

    static open(authorMapJson = "{}"): Gitmoji {
        const native = loadNiftyNative();
        return new Gitmoji(native.gitmoji, authorMapJson);
    }

    knownGitmojis(): string[] {
        return this.wasm["known-gitmojis"]();
    }

    validateSubject(subject: string): boolean {
        return this.wasm["validate-subject"](subject);
    }

    parseSubject(subject: string): ParsedSubject {
        return this.wasm["parse-subject"](subject);
    }

    formatSubject(gitmoji: string, body: string): string {
        return this.wasm["format-subject"](gitmoji, body);
    }

    stripGitmoji(subject: string): string {
        return this.wasm["strip-gitmoji"](subject);
    }

    leadingGitmoji(subject: string): string | undefined {
        return this.wasm["leading-gitmoji"](subject);
    }

    sectionForGitmoji(gitmoji?: string): ReleaseSection {
        return this.wasm["section-for-gitmoji"](gitmoji);
    }

    parseNoreplyEmail(email: string): GithubAuthor | undefined {
        return this.wasm["parse-noreply-email"](email);
    }

    resolveGithubAuthor(email: string, authorMapJson = this.defaultAuthorMapJson): GithubAuthor | undefined {
        return this.wasm["resolve-github-author"](email, authorMapJson);
    }

    displayLogin(author: GithubAuthor): string {
        return this.wasm["display-login"](author);
    }

    profileUrl(author: GithubAuthor): string {
        return this.wasm["profile-url"](author);
    }

    avatarUrl(author: GithubAuthor): string {
        return this.wasm["avatar-url"](author);
    }

    authorMention(email: string, authorName: string, authorMapJson = this.defaultAuthorMapJson): string {
        return this.wasm["author-mention"](email, authorName, authorMapJson);
    }

    commitBullet(body: string, email: string, authorName: string, authorMapJson = this.defaultAuthorMapJson): string {
        return this.wasm["commit-bullet"](body, email, authorName, authorMapJson);
    }
}

export function createGitmoji(): Gitmoji {
    return Gitmoji.open();
}
