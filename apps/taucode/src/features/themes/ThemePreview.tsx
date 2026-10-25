import type { CSSProperties } from 'react';
import { ArrowUp, Check, Folder, Sparkles } from 'lucide-react';
import { resolveTheme } from './builtins';
import type { ThemeManifest } from './validate';
import { contrastPairs } from './contrast';

export function ThemePreview({ theme, compact = false, selected = false }: { theme: ThemeManifest; compact?: boolean; selected?: boolean }) {
  const resolved = resolveTheme(theme);
  const style = {
    ...resolved.tokens,
    '--preview-ui': resolved.fonts?.ui ?? 'system-ui, sans-serif',
    '--preview-mono': resolved.fonts?.mono ?? 'ui-monospace, monospace',
    '--preview-prose': resolved.fonts?.prose ?? resolved.fonts?.ui ?? 'system-ui, sans-serif',
  } as CSSProperties;
  if (compact) return <span className="theme-mini" data-glass={!!resolved.window?.translucent} data-neon-glow={!!resolved.effects?.neonGlow} style={style} aria-hidden="true">
    <span className="theme-mini-sidebar"><span className="theme-mini-project" /><span className="theme-mini-selected" /><span className="theme-mini-tree" /></span>
    <span className="theme-mini-content"><span className="theme-mini-message">Hello, world</span><span className="theme-mini-prose">Aa</span><span className="theme-mini-button">Send <ArrowUp size={11} /></span></span>
    {selected && <span className="theme-check"><Check size={13} /></span>}
  </span>;
  return <div className="theme-preview-column">
    <h3>Live preview</h3>
    <div className="theme-app-preview" data-glass={!!resolved.window?.translucent} data-neon-glow={!!resolved.effects?.neonGlow} style={style} data-testid="theme-preview">
      <aside className="theme-preview-sidebar"><span className="theme-preview-brand"><Sparkles size={16} /> taucode</span><div className="theme-preview-project"><Folder size={15} /> My project</div><div className="theme-preview-tree"><span>Explore an idea</span><span className="theme-preview-selected">A fresh start</span><span>Review changes</span></div></aside>
      <div className="theme-preview-main"><div className="theme-preview-title">A fresh start</div><div className="theme-preview-conversation"><div className="theme-preview-user">Make something thoughtful.</div><div className="theme-preview-assistant"><strong>Aa — a little room to think</strong><p>Clear ideas start with a comfortable space. This is how your words will feel.</p></div><pre><code>{'const greeting = "Hello";\nconsole.log(greeting);'}</code></pre></div><div className="theme-preview-composer"><span>Ask a question…</span><span className="theme-preview-send"><ArrowUp size={16} /></span></div></div>
    </div>
    <div className="theme-contrast"><h3>Contrast <small>Target ≥ 4.5:1</small></h3><ul>{contrastPairs(resolved.tokens, resolved.window?.translucent ? resolved.appearance : undefined).map(pair => <li key={pair.text+pair.surface}><span>{pair.label}</span><span className="theme-contrast-ratio">{pair.ratio === null ? '—' : pair.ratio.toFixed(2)+':1'}</span><span className={'theme-contrast-badge '+(pair.ratio !== null && pair.ratio >= 4.5 ? 'is-pass' : 'is-fail')}>{pair.ratio === null ? 'Invalid' : pair.ratio >= 4.5 ? 'Pass' : 'Fail'}</span></li>)}</ul></div>
  </div>;
}
