import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';

import './i18n';
import { App } from './App';
import { patchConsoleForwarding } from './lib/logger';
import './styles.css';

const container = document.getElementById('root');

if (!container) {
  throw new Error('Missing root container');
}

patchConsoleForwarding();

createRoot(container).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
