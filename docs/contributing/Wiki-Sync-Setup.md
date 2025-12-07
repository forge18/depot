# Wiki Sync Setup Guide

This guide will help you set up automatic wiki synchronization for the LPM project.

## Quick Setup (5 minutes)

### Step 1: Generate a Personal Access Token

1. Go to: https://github.com/settings/tokens
2. Click **"Generate new token"** → **"Generate new token (classic)"**
3. Fill in the form:
   - **Note:** `LPM Wiki Sync` (or any descriptive name)
   - **Expiration:** Choose your preference (90 days, 1 year, or no expiration)
   - **Scopes:** Check the `repo` checkbox (this gives "Full control of private repositories")
4. Scroll down and click **"Generate token"**
5. **IMPORTANT:** Copy the token immediately - you won't be able to see it again!

### Step 2: Add Token as Repository Secret

1. Go to your LPM repository on GitHub
2. Click **Settings** (top menu)
3. In the left sidebar, click **Secrets and variables** → **Actions**
4. Click **"New repository secret"**
5. Fill in:
   - **Name:** `GH_PERSONAL_ACCESS_TOKEN` (must be exactly this)
   - **Secret:** Paste your token from Step 1
6. Click **"Add secret"**

### Step 3: Verify Setup

1. Go to the **Actions** tab in your repository
2. Find the **"Sync Documentation to Wiki"** workflow
3. Click **"Run workflow"** → **"Run workflow"** (to test manually)
4. Check the workflow run - it should complete successfully

## What Happens Next

Once the token is set up:

- **Automatic sync:** When you push changes to `docs/user/` or `docs/contributing/`, the wiki will automatically update
- **Manual sync:** You can trigger it manually from the Actions tab anytime
- **No more errors:** The workflow will run successfully instead of showing warnings

## Troubleshooting

### Token not working?

- Make sure the token has the `repo` scope
- Verify the secret name is exactly `GH_PERSONAL_ACCESS_TOKEN` (case-sensitive)
- Check that the token hasn't expired
- Try generating a new token if needed

### Wiki not updating?

- Check the Actions tab for workflow run logs
- Verify the wiki is enabled for your repository (Settings → Features → Wiki)
- Make sure you're pushing to the `main` or `master` branch

### Still seeing errors?

- The workflow now handles missing tokens gracefully
- If you see warnings, it means the token isn't set (which is fine - wiki sync is optional)
- If you want to enable it, follow the steps above

## Security Notes

- **Never commit tokens to git** - Always use GitHub Secrets
- **Use minimal scopes** - The `repo` scope is needed for wiki access
- **Rotate tokens periodically** - Update the secret if you regenerate the token
- **Revoke unused tokens** - Delete old tokens you're no longer using

## Optional: Fine-Tune Token Scopes

If you want to be more restrictive (though `repo` is the minimum needed):

- The token needs write access to the repository
- `repo` scope includes wiki write access
- More granular scopes may not work with the wiki action

## Need Help?

- Check the workflow logs in the Actions tab
- See `docs/contributing/GitHub-Actions.md` for more details
- The workflow will show helpful error messages if something goes wrong

