import type { Route } from "./+types/home";
import { useState } from 'react';

export function meta({}: Route.MetaArgs) {
  return [
    { title: "New React Router App" },
    { name: "description", content: "Welcome to React Router!" },
  ];
}

function Counter() {
  const [score, setScore] = useState(0);

  function increment() {
    setScore(s => s + 1);
  }

  return (
    <>
      <style>
        {`
          button {
            margin: 5px;
          }
        `}
      </style>
      <button onClick={() => increment()}>+1</button>
      <button onClick={() => {
        increment();
        increment();
        increment();
      }}>+3</button>
      <h1>Score: {score}</h1>
    </>
  )
}

export default function Home() {
  return (
    <>
      <h1>Home</h1>
      <Counter />
      <Counter />
    </>
  )
}
