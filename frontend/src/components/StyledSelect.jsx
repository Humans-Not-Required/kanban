// ---- Styled Select (custom chevron, consistent across platforms) ----
// Uses wrapper <div> with background-image SVG chevron.
// Inject global CSS once for select appearance reset.
if (typeof document !== 'undefined' && !document.getElementById('styled-select-css')) {
  const _ssStyle = document.createElement('style');
  _ssStyle.id = 'styled-select-css';
  _ssStyle.textContent = `
    .ss-wrap { display: inline-flex; }
    .ss-wrap select {
      -webkit-appearance: none !important;
      -moz-appearance: none !important;
      appearance: none !important;
    }
    .ss-wrap select::-ms-expand { display: none; }
  `;
  document.head.appendChild(_ssStyle);
}

const CHEVRON_SVG = "url(\"data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='12' height='8' viewBox='0 0 12 8' fill='none'%3E%3Cpath d='M1.5 1.75L6 6.25L10.5 1.75' stroke='%2394a3b8' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'/%3E%3C/svg%3E\")";

export default function StyledSelect({ style, children, ...props }) {
  const { background, backgroundImage: _bi, flex, minWidth, width, gridColumn, ...restStyle } = style || {};
  const wrapStyle = {
    flex: flex,
    minWidth: minWidth,
    width: width,
    gridColumn: gridColumn,
  };
  const selectStyle = {
    ...restStyle,
    backgroundColor: restStyle.backgroundColor || background || 'transparent',
    paddingRight: '40px',
    cursor: 'pointer',
    width: '100%',
    flex: 1,
    backgroundImage: CHEVRON_SVG,
    backgroundRepeat: 'no-repeat',
    backgroundPosition: 'right 12px center',
    backgroundSize: '12px 8px',
  };
  return (
    <div className="ss-wrap" style={wrapStyle}>
      <select style={selectStyle} {...props}>
        {children}
      </select>
    </div>
  );
}
