import type { BaseLayoutProps } from 'fumadocs-ui/layouts/shared';
import { appName, gitConfig } from './shared';

export function baseOptions(): BaseLayoutProps {
  return {
    nav: {
      title: appName,
      transparentMode: 'top',
      children: (
        <a className="family-switcher" href="https://miguelcsx.github.io/molframe/">
          <span>Product family</span>
          <strong>MolFrame ↗</strong>
        </a>
      ),
    },
    githubUrl: `https://github.com/${gitConfig.user}/${gitConfig.repo}`,
  };
}
