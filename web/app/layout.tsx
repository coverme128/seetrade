import type { Metadata } from "next";
import "./globals.css";
export const metadata: Metadata = {
  title: "Micro Alpha · 个人量化工作台",
  description: "短周期微观结构观察、模拟交易与模型验证",
};
export default function RootLayout({
  children,
}: Readonly<{ children: React.ReactNode }>) {
  return (
    <html lang="zh-CN">
      <body>{children}</body>
    </html>
  );
}
