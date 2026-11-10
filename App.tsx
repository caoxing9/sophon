// Sophon's entry: the Rust app (`src/lib.rs`, `sophon/native`) with the
// screens written in TSX, which it places where they belong.
import { mount } from "nsh";
import { App } from "sophon/native";
import { General } from "./tsx/settings/General";

mount(() => <App renderGeneral={() => <General />} />);
