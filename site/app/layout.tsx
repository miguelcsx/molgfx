import { Provider } from '@/components/provider';
import './global.css';

export const metadata = {
  metadataBase: new URL('https://miguelcsx.github.io/molgfx/'),
  title: { default: 'MolGFX Documentation', template: '%s | MolGFX' },
  description: 'End-user documentation for MolGFX molecular visualization.',
};

export default function Layout({ children }: LayoutProps<'/'>) {
  return (
    <html lang="en" suppressHydrationWarning>
      <body className="flex min-h-screen flex-col">
        <Provider>{children}</Provider>
      </body>
    </html>
  );
}
