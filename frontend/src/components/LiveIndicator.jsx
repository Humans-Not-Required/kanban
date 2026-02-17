export default function LiveIndicator({ status, isMobile }) {
  if (!status || status === 'initial') return null;
  const connected = status === 'connected';
  const color = connected ? '#22c55e' : '#ef4444';
  const title = connected ? 'Live — real-time sync active' : 'Reconnecting…';

  if (isMobile) {
    return (
      <span title={title} style={{
        display: 'inline-flex', alignItems: 'center', gap: '5px',
        cursor: 'default',
      }}>
        <span style={{
          width: '7px', height: '7px',
          borderRadius: '50%', background: color, flexShrink: 0,
          boxShadow: connected ? `0 0 5px ${color}80` : 'none',
          animation: connected ? 'ssePulse 2.5s ease-in-out infinite' : 'none',
        }} />
        {!connected && (
          <span style={{ fontSize: '0.6rem', color: '#fca5a5', fontWeight: 500, whiteSpace: 'nowrap' }}>
            Reconnecting…
          </span>
        )}
      </span>
    );
  }

  return (
    <span title={title} style={{
      display: 'inline-flex', alignItems: 'center', gap: '5px',
      cursor: 'default',
      background: connected ? '#22c55e18' : '#ef444418',
      border: `1px solid ${connected ? '#22c55e40' : '#ef444440'}`,
      borderRadius: '9999px',
      padding: '2px 8px 2px 6px',
      fontSize: '0.65rem',
      fontWeight: 600,
      letterSpacing: '0.04em',
      color: connected ? '#4ade80' : '#fca5a5',
      textTransform: 'uppercase',
      whiteSpace: 'nowrap',
    }}>
      <span style={{
        width: '6px', height: '6px',
        borderRadius: '50%', background: color, flexShrink: 0,
        boxShadow: connected ? `0 0 4px ${color}80` : 'none',
        animation: connected ? 'ssePulse 2.5s ease-in-out infinite' : 'none',
      }} />
      {connected ? 'Live' : 'Reconnecting…'}
    </span>
  );
}
