// Dispatch demo: the real interface in a plain browser, with a stand-in for
// the Rust side that answers every call with made-up demo data. Used for the
// README screenshots and the video (HANDOFF.md, "Demo mode"). Dev only: it
// isn't one of Vite's build inputs, so it never ships in the app.
//
//   bunx vite dev, then open http://localhost:1420/src/dispatch-demo/
//
// The stand-in has to be in place before the app's first call, so it's
// imported first: modules run in import order.
import "./mockBackend";
import "../main";
