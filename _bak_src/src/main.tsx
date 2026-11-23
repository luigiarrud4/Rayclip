import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./styles.css";

// Sem menu nativo do WebView2: o botão direito abre o painel de ações.
window.addEventListener("contextmenu", (e) => e.preventDefault());

/** Mostra o erro na tela em vez de deixar a janela em branco. */
class Boundary extends React.Component<{ children: React.ReactNode }, { err: string | null }> {
  state = { err: null as string | null };
  static getDerivedStateFromError(e: unknown) {
    return { err: e instanceof Error ? `${e.message}
${e.stack ?? ""}` : String(e) };
  }
  render() {
    if (this.state.err) {
      return (
        <div style={{ color: "#ff6363", font: "11px/1.5 monospace", padding: 10, whiteSpace: "pre-wrap" }}>
          UI error: {this.state.err}
        </div>
      );
    }
    return this.props.children;
  }
}

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <Boundary>
      <App />
    </Boundary>
  </React.StrictMode>
);
