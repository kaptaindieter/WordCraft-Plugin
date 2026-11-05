#!/usr/bin/env python3
"""
Craft Suite Plugin Uninstaller for WordCraft
Cleanly removes the Craft Suite Plugin files and reverses the integration hooks,
restoring the vanilla WordCraft repository to its original state.
"""

import argparse
import os
import shutil
import subprocess
import sys

PLUGIN_ROOT = os.path.dirname(os.path.abspath(__file__))
STANDALONE_MODULES = ['artcraft_agent.rs', 'artcraft_mcp.rs', 'artcraft_drive.rs', 'artcraft_theme.rs']


def uninstall_plugin(target_dir: str) -> int:
    target_dir = os.path.abspath(target_dir)
    ui_egui_src = os.path.join(target_dir, "crates", "ui-egui", "src")
    if not os.path.isdir(ui_egui_src):
        print(f"[ERROR] Target directory is not a valid WordCraft repository: {target_dir}")
        return 1

    print(f"=== Uninstalling Craft Suite Plugin from WordCraft ({target_dir}) ===")

    # 1. Reverse vanilla integration hooks patch
    patch_file = os.path.join(PLUGIN_ROOT, "patches", "vanilla-hooks.patch")
    legacy_patch = os.path.join(PLUGIN_ROOT, "patches", "vanilla-hooks-legacy.patch")
    reverted = False
    for pf in [patch_file, legacy_patch]:
        if os.path.isfile(pf):
            rev_cmd = subprocess.run(
                ["git", "apply", "--reverse", "--ignore-space-change", "--ignore-whitespace", pf],
                cwd=target_dir,
                capture_output=True,
                text=True,
            )
            if rev_cmd.returncode == 0:
                print(f"  [-] Reversed vanilla integration hooks ({os.path.basename(pf)})")
                reverted = True
                break
    if not reverted:
        print("  [=] Vanilla integration hooks were not applied or already reverted.")

    # 2. Remove standalone Rust plugin modules
    for mod_name in STANDALONE_MODULES:
        dst_path = os.path.join(ui_egui_src, mod_name)
        if os.path.isfile(dst_path):
            os.remove(dst_path)
            print(f"  [-] Removed plugin module: crates/ui-egui/src/{mod_name}")

    # 3. Remove AI provider icons
    target_icons_dir = os.path.join(target_dir, "assets", "ai_icons")
    if os.path.isdir(target_icons_dir):
        shutil.rmtree(target_icons_dir)
        print("  [-] Removed AI provider icons: assets/ai_icons/")

    print("\n[SUCCESS] Craft Suite Plugin cleanly uninstalled. Vanilla repository restored.")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description="Uninstall the Craft Suite Plugin from a WordCraft checkout.")
    parser.add_argument("target", help="Path to the WordCraft repository root")
    args = parser.parse_args()
    return uninstall_plugin(args.target)


if __name__ == "__main__":
    sys.exit(main())
