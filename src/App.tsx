import "./App.css";
import { MainView } from "./MainView";
import { SettingsView } from "./SettingsView";

function App() {
  const params = new URLSearchParams(window.location.search);
  const view = params.get("view");

  return view === "settings" ? <SettingsView /> : <MainView />;
}

export default App;
