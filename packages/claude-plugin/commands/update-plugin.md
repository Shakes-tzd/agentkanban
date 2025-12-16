# Update Ijoka Plugin

Run the update script to commit plugin changes to GitHub and reinstall from GitHub:

```bash
/Users/shakes/DevProjects/ijoka/scripts/update-claude-plugin.sh
```

This script will:
1. Check for uncommitted changes in `packages/claude-plugin/` and `.claude-plugin/`
2. If changes exist, commit and push them to GitHub
3. Ensure the ijoka marketplace is configured (from `Shakes-tzd/ijoka`)
4. Update the marketplace (pull latest from GitHub)
5. Reinstall the plugin **from GitHub** (not local)

**Optional:** Provide a custom commit message:
```bash
/Users/shakes/DevProjects/ijoka/scripts/update-claude-plugin.sh "feat(plugin): add new feature"
```

**Why GitHub?** Installing from GitHub ensures you're always running the committed version, catching any issues where you forgot to push changes.

**Important:** After running this script, you may need to restart Claude Code for changes to take full effect.

Run the script above using the Bash tool.
