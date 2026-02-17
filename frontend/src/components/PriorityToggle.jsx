import { PRIORITY_OPTIONS } from '../utils';

export default function PriorityToggle({ value, onChange, compact = false }) {
  return (
    <div style={{
      display: 'flex',
      borderRadius: '6px',
      overflow: 'hidden',
      border: '1px solid #475569',
      flex: 1,
      minHeight: '32px',
      boxSizing: 'border-box',
    }}>
      {PRIORITY_OPTIONS.map((opt, i) => {
        const isActive = value === opt.value;
        return (
          <button
            key={opt.value}
            type="button"
            onClick={() => onChange(opt.value)}
            aria-label={opt.label}
            title={opt.label}
            style={{
              flex: 1,
              padding: compact ? 0 : '6px 0',
              fontSize: compact ? '0.75rem' : '0.78rem',
              fontWeight: isActive ? '700' : '500',
              color: isActive ? '#fff' : '#94a3b8',
              background: isActive ? opt.color + 'cc' : '#1e293b',
              border: 'none',
              borderRight: i < PRIORITY_OPTIONS.length - 1 ? '1px solid #475569' : 'none',
              cursor: 'pointer',
              transition: 'background 0.15s, color 0.15s',
              lineHeight: '1.2',
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
              userSelect: 'none',
            }}
          >
            {compact ? (
              <span
                style={{
                  width: '12px',
                  height: '12px',
                  borderRadius: '999px',
                  background: opt.color,
                  boxShadow: isActive ? '0 0 0 2px rgba(255,255,255,0.35)' : '0 0 0 1px rgba(0,0,0,0.35)',
                }}
              />
            ) : (
              opt.label
            )}
          </button>
        );
      })}
    </div>
  );
}
