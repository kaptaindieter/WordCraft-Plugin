#!/usr/bin/env python3
"""
Craft Suite Plugin Installer for WordCraft
Applies the standalone Craft Suite Plugin (BYOK AI Agent, Dynamic Cloud Sync,
and Aurora/Dark/Light Theme System) onto a vanilla WordCraft checkout.
"""

import argparse
import os
import shutil
import subprocess
import sys

PLUGIN_ROOT = os.path.dirname(os.path.abspath(__file__))
STANDALONE_MODULES = ['artcraft_agent.rs', 'artcraft_mcp.rs', 'artcraft_drive.rs', 'artcraft_theme.rs']


def apply_plugin(target_dir: str, build: bool = False) -> int:
    target_dir = os.path.abspath(target_dir)
    ui_egui_src = os.path.join(target_dir, "crates", "ui-egui", "src")
    if not os.path.isdir(ui_egui_src):
        print(f"[ERROR] Target directory does not look like a valid WordCraft repository:")
        print(f"        Missing directory: {ui_egui_src}")
        return 1

    print(f"=== Installing Craft Suite Plugin into WordCraft ({target_dir}) ===")

    # 1. Copy standalone Rust plugin modules
    plugin_src_dir = os.path.join(PLUGIN_ROOT, "plugin", "src")
    for mod_name in STANDALONE_MODULES:
        src_path = os.path.join(plugin_src_dir, mod_name)
        dst_path = os.path.join(ui_egui_src, mod_name)
        shutil.copy2(src_path, dst_path)
        print(f"  [+] Installed plugin module: crates/ui-egui/src/{mod_name}")

    # 2. Copy AI provider icon assets
    plugin_icons_dir = os.path.join(PLUGIN_ROOT, "plugin", "assets", "ai_icons")
    target_icons_dir = os.path.join(target_dir, "assets", "ai_icons")
    os.makedirs(target_icons_dir, exist_ok=True)
    for fname in os.listdir(plugin_icons_dir):
        shutil.copy2(
            os.path.join(plugin_icons_dir, fname),
            os.path.join(target_icons_dir, fname),
        )
    print(f"  [+] Installed AI provider icons into: assets/ai_icons/")

    # 3. Apply vanilla integration hooks patch (supports latest upstream/main and legacy commits)
    patch_file = os.path.join(PLUGIN_ROOT, "patches", "vanilla-hooks.patch")
    legacy_patch = os.path.join(PLUGIN_ROOT, "patches", "vanilla-hooks-legacy.patch")
    candidates = [p for p in [patch_file, legacy_patch] if os.path.isfile(p)]
    if candidates:
        already = False
        for pf in candidates:
            rev_check = subprocess.run(
                ["git", "apply", "--reverse", "--check", "--ignore-space-change", "--ignore-whitespace", pf],
                cwd=target_dir,
                capture_output=True,
                text=True,
            )
            if rev_check.returncode == 0:
                already = True
                break
        if already:
            print("  [=] Vanilla integration hooks are already applied.")
        else:
            applied = False
            for pf in candidates:
                chk = subprocess.run(
                    ["git", "apply", "--check", "--ignore-space-change", "--ignore-whitespace", pf],
                    cwd=target_dir,
                    capture_output=True,
                    text=True,
                )
                if chk.returncode == 0:
                    subprocess.run(
                        ["git", "apply", "--ignore-space-change", "--ignore-whitespace", pf],
                        cwd=target_dir,
                        check=True,
                    )
                    print(f"  [+] Applied vanilla integration hooks ({os.path.basename(pf)})")
                    applied = True
                    break
            if not applied:
                print("  [~] Trying 3-way merge (--3way)...")
                for pf in candidates:
                    apply_3way = subprocess.run(
                        ["git", "apply", "--3way", "--ignore-space-change", "--ignore-whitespace", pf],
                        cwd=target_dir,
                        capture_output=True,
                        text=True,
                    )
                    if apply_3way.returncode == 0:
                        print(f"  [+] Applied vanilla integration hooks via 3-way merge ({os.path.basename(pf)})")
                        applied = True
                        break
                if not applied:
                    print("[ERROR] Failed to apply vanilla integration hooks.")
                    return 2

    print("\n[SUCCESS] Craft Suite Plugin installed successfully!")
    print("  - Cloud Sync folder can be chosen directly inside the app header ('Cloud & Auto-Save Sync').")
    print("  - BYOK API keys can be added inside the 'AI Agent' sidebar ('+ API' / 'Manage APIs').")

    if build:
        print(f"\n=== Building WordCraft (cargo build --release -p wordcraft) ===")
        res = subprocess.run(["cargo", "build", "--release", "-p", "wordcraft"], cwd=target_dir)
        return res.returncode

    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description="Install the Craft Suite Plugin into a vanilla WordCraft checkout.")
    parser.add_argument("target", help="Path to the vanilla WordCraft repository root")
    parser.add_argument("--build", action="store_true", help="Compile the release binary after installing the plugin")
    args = parser.parse_args()
    return apply_plugin(args.target, args.build)


if __name__ == "__main__":
    sys.exit(main())
