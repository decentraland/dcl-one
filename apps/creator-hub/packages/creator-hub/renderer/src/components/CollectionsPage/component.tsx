import { Navbar, NavbarItem } from '../Navbar';

import styles from './component.module.css';

import './styles.css';

export function CollectionsPage() {
  return (
    <main className="CollectionsPage">
      <Navbar active={NavbarItem.COLLECTIONS} />
      <div className={styles.container}>WIP</div>
    </main>
  );
}
