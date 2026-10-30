import { defineConfig } from 'vitepress'

const repo = 'https://github.com/aiden0rchad/taucode'

export default defineConfig({
  lang: 'en-US',
  title: 'taucode',
  titleTemplate: ':title | taucode Docs',
  description: 'A desktop workspace for AI coding, conversations, and document work. Bring your own models and choose your engine.',
  base: '/taucode/',
  appearance: 'dark',
  cleanUrls: true,
  lastUpdated: true,
  srcExclude: ['specs/**', 'fixtures/**', 'PLAN.md', 'dogfood-acceptance.md'],
  sitemap: { hostname: 'https://aiden0rchad.github.io/taucode/' },
  head: [
    ['link', { rel: 'icon', type: 'image/svg+xml', href: '/taucode/taucode-mark.svg' }],
    ['meta', { name: 'theme-color', content: '#101017' }],
  ],
  markdown: { lineNumbers: true },
  themeConfig: {
    logo: { src: '/taucode-mark.svg', alt: 'taucode' },
    siteTitle: 'taucode / docs',
    nav: [
      { text: 'Get started', link: '/getting-started' },
      { text: 'Guides', link: '/code' },
      { text: 'FAQ', link: '/faq' },
      { text: 'Builds & status', link: '/releases' },
    ],
    sidebar: [
      { text: 'Start here', items: [
        { text: 'Welcome', link: '/' },
        { text: 'Installation & first run', link: '/getting-started' },
        { text: 'Providers & models', link: '/providers' },
        { text: 'Engines & feature switches', link: '/engines' },
        { text: 'Builds & platform status', link: '/releases' },
      ] },
      { text: 'Use taucode', items: [
        { text: 'Code · projects & conversations', link: '/code' },
        { text: 'Chat · conversations & attachments', link: '/chat' },
        { text: 'Work · spaces, tasks & outputs', link: '/work' },
        { text: 'Agents & Chain of Command', link: '/agents' },
        { text: 'Checkpoints & rewind', link: '/checkpoints' },
      ] },
      { text: 'Connect & customize', items: [
        { text: 'MCP connectors & CodeGraph', link: '/connectors' },
        { text: 'Browser', link: '/browser' },
        { text: 'Computer use & permissions', link: '/computer-use' },
        { text: 'Themes & appearance', link: '/themes' },
        { text: 'Remote daemon & iOS', link: '/remote' },
      ] },
      { text: 'Understand & troubleshoot', items: [
        { text: 'Privacy, storage & trust', link: '/privacy' },
        { text: 'Troubleshooting', link: '/troubleshooting' },
        { text: 'Frequently asked questions', link: '/faq' },
        { text: 'Diagnostics journal', link: '/diagnostics' },
        { text: 'Evals & model comparisons', link: '/evals' },
      ] },
      { text: 'Build & contribute', collapsed: false, items: [
        { text: 'Development', link: '/development' },
        { text: 'Architecture & source map', link: '/architecture' },
        { text: 'macOS packaging & signing', link: '/packaging' },
        { text: 'Engineering references', link: '/engineering' },
        { text: 'Credits & third-party notices', link: '/THIRD_PARTY_NOTICES' },
      ] },
    ],
    search: { provider: 'local' },
    outline: { level: [2, 3], label: 'On this page' },
    editLink: { pattern: `${repo}/edit/main/docs/:path`, text: 'Improve this page on GitHub' },
    lastUpdated: { text: 'Last updated', formatOptions: { dateStyle: 'medium' } },
    socialLinks: [{ icon: 'github', link: repo }],
    footer: {
      message: `MIT licensed · <a href="${repo}/blob/main/docs/THIRD_PARTY_NOTICES.md">Credits & third-party notices</a>`,
      copyright: 'taucode contributors · Built on CodexMonitor',
    },
    darkModeSwitchLabel: 'Theme',
    sidebarMenuLabel: 'Documentation menu',
    returnToTopLabel: 'Back to top',
    externalLinkIcon: true,
  },
})
