import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import BallApp from "./windows/BallApp";
import OverlayApp from "./windows/OverlayApp";
import PanelApp from "./windows/PanelApp";
import PinApp from "./windows/PinApp";
import { windowLabel } from "./api";
import "./styles/index.css";

/**
 * 所有窗口共用一个 HTML 入口，按窗口 label 分发。
 * 好处：不用为每个窗口配一份 Vite 多入口构建。
 */
const label = windowLabel();
document.documentElement.dataset.window = label;

/** 按窗口 label 选根组件 */
const root =
  label === "ball" ? <BallApp />
  : label === "main" ? <PanelApp />
  : label === "overlay" ? <OverlayApp />
  : label.startsWith("pin-") ? <PinApp />
  : <div className="unknown-window">未知窗口：{label}</div>;

const container = document.getElementById("root");
if (!container) throw new Error("#root 不存在");

createRoot(container).render(<StrictMode>{root}</StrictMode>);
