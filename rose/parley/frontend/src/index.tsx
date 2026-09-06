import { render } from "solid-js/web";
import { Route, Router } from "@solidjs/router";
import App from "./App";
import Converse from "./pages/Converse";
import Progress from "./pages/Progress";
import Vocabulary from "./pages/Vocabulary";
import Planned from "./pages/Planned";

// App is the shell (nav + layout); each Route renders into it. Converse is home,
// Progress reads the learner-state spine, and /f/:id shows a planned feature's theory.
render(
  () => (
    <Router root={App}>
      <Route path="/" component={Converse} />
      <Route path="/progress" component={Progress} />
      <Route path="/vocabulary" component={Vocabulary} />
      <Route path="/f/:id" component={Planned} />
    </Router>
  ),
  document.getElementById("root")!,
);
