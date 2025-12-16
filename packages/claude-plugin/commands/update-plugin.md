# Update Ijoka Plugin

Run the update script to commit any plugin changes to GitHub and reinstall the plugin:

```bash
/Users/shakes/DevProjects/ijoka/scripts/update-claude-plugin.sh
```

This script will:
1. Check for uncommitted changes in `packages/claude-plugin/`
2. If changes exist, commit and push them to GitHub
3. Clean the plugin cache
4. Reinstall the plugin from the local directory

**Optional:** Provide a custom commit message:
```bash
/Users/shakes/DevProjects/ijoka/scripts/update-claude-plugin.sh "feat(plugin): add new feature"
```

**Important:** After running this script, you may need to restart Claude Code for changes to take full effect.

Run the script above using the Bash tool.
