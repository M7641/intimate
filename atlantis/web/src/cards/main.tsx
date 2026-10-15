/* @refresh reload */
import { render } from "solid-js/web";
import { CardsApp } from "./CardsApp";
import "../style.css";
import "./cards.css";

render(() => <CardsApp />, document.getElementById("root")!);
