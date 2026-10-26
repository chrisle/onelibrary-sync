import { SyncPane } from "@/views/SyncPane";
import styles from "./App.module.css";

export function App() {
  return <main className={styles.window}>
    <header className={styles.titleBar} data-tauri-drag-region>
      <span>OneLibrary Sync</span>
    </header>
    <div className={styles.scroller}>
      <SyncPane />
    </div>
  </main>;
}
