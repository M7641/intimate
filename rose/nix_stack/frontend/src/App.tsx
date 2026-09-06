import { createResource, type Component } from "solid-js";

type InfoResponse = {
  name: string;
  description: string;
  stack: string[];
};

const fetchInfo = async (): Promise<InfoResponse> => {
  const res = await fetch("/api/info");
  return res.json();
};

const App: Component = () => {
  const [info] = createResource(fetchInfo);

  return (
    <main style={{ "font-family": "system-ui", padding: "2rem" }}>
      <h1>nix_stack</h1>
      <p>Nix-reproducible multi-language development environment</p>

      {info.loading && <p>Loading...</p>}
      {info.error && (
        <p style={{ color: "red" }}>Error: {info.error.message}</p>
      )}
      {info() && (
        <div>
          <h2>{info()!.name}</h2>
          <p>{info()!.description}</p>
          <h3>Stack</h3>
          <ul>
            {info()!.stack.map((item) => (
              <li>{item}</li>
            ))}
          </ul>
        </div>
      )}
    </main>
  );
};

export default App;
