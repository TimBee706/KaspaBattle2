// Central, typed source of truth for every externally-facing project link.
// Do not hardcode any of these URLs again in components — import from here.
//
// Verification status (see DEPLOYMENT/PR notes for the full check):
// - website, githubProfile, githubRepository, githubIssues: verified via `git remote -v`
//   (origin = https://github.com/TimBee706/KaspaBattle2.git) and a live profile/repo fetch.
// - x, instagram: verified reachable and on-topic via a live browser check.
// - reddit: could NOT be verified (Reddit is unreachable from this environment's tools).
//   Present here so it's easy to enable once confirmed, but `verified: false` means
//   components should not render it prominently by default — see SocialLinks.tsx.

export interface PublicLink {
    url: string;
    verified: boolean;
}

export const publicLinks = {
    website: 'https://www.kaspabattle.com/',
    githubProfile: 'https://github.com/TimBee706',
    githubRepository: 'https://github.com/TimBee706/KaspaBattle2',
    githubIssues: 'https://github.com/TimBee706/KaspaBattle2/issues',
    githubIssueNew: 'https://github.com/TimBee706/KaspaBattle2/issues/new/choose',
    githubSecurityAdvisories: 'https://github.com/TimBee706/KaspaBattle2/security/advisories/new',
    x: 'https://x.com/KaspaBattle',
    instagram: 'https://www.instagram.com/kaspabattle/',
    reddit: 'https://www.reddit.com/user/KaspaBattle/',
} as const;

// Social links shown in SocialLinks/Footer, with verification status attached.
// `reddit` is intentionally excluded from prominent display until independently
// confirmed — see the note above.
export const socialLinks: Record<'x' | 'instagram' | 'reddit', PublicLink> = {
    x: { url: publicLinks.x, verified: true },
    instagram: { url: publicLinks.instagram, verified: true },
    reddit: { url: publicLinks.reddit, verified: false },
};

export type PublicLinkKey = keyof typeof publicLinks;
