# Git

I was reflecting on why I've never really felt the need to be good at git. I don't believe the value of git aligned with what I was traditionally trying to achieve, and only now is that starting to change. 

The first concept I want to put into words is how GitHub and Git differ. git is the software that runs on your computer, and GitHub is a website that stores the folders you use with git so you can share those files with other people and computers.

Git is a method for saving snapshots of a codebase over time. A comparison with video games does get made, where you can save your progress and reload at will. Also, you can stretch the analogy: you can save files with others, who can then pick up the game where you left off.

Given this concept, the purpose of git is simply to create sensible save points in your codebase. Before a feature is made or a large refactor is done is an ideal location to have a save point. Then the message makes it easy for future you to know what that point was. 

Branching allows others to take a codebase and add their own changes without affecting the main version. It also provides a way to maintain a version of the codebase that does not pass all the tests whilst you work on a significant change. This point is important for CI/CD practices.

Then, merging is the act of reconciling those differences back into a single branch. 

Being good at git means being good at managing the history of a set of save points for a codebase.

In that context, we can also talk about rebasing. The normal git methods, such as commit, push, and merge, are all additive to the commit history. Rebase lets you change the past.

This is an alternative route to merging, where you take the branch you are on and rebase it onto main. This will collect all the diffs on that branch, apply them to main, and then apply the current branch's diffs on top, as if the branch were the next commit on main. You would then need to merge this new branch into main to update main with the new branch's code. 

You end up in the same place as a merge, but now there's only the commit with the branch code, and all the rest has been cut out of the history. 

This is useful when you want to submit a PR with a linear commit history relative to main. 

Rebasing is more dangerous because you are tampering with previous commits. If someone else is working on those commits and you rebase them, you create a mess with their work, with commits that no longer exist.

Merging vs rebasing is like keeping a record of the full history as a ledger or telling the history of a project as if it were a story. Merging creates a messier history, but it is a true one, whereas rebasing is more of a defined story. 

I somewhat like the idea of creating a branch, adding a feature, then rebasing that branch into a single commit with a rebase squash and a clean message that can be Claude-generated. 

Rebasing is also how you can redefine the history of a project, which might be worth doing one day if you are happy to remove much of the past. 

Either way, that is git; it's a tool to create and manage the history of a codebase. What you do with those tools is up to you. 

## GitHub 

GitHub is a website that hosts your git for you. This fundamentally lets you share your git history with other users. 

Users can also be production servers as well.

This is when you can start to have conversations about how you then use git to achieve other objectives such as CI and CD. 

Let's first split these into two topics: Continuous Integration (CI) and then Continuous Deployment (CD).

Continuous Integration is the practice of integrating changes into the main version of the codebase. That is not to have long, drawn-out branches that diverge from the source version of the code. Instead, the branches are either kept incredibly up to date with the main branch or are constantly merged back in. 

Why is this a good thing? The proposition is that, in essence, creating a distinct codebase and then merging it back in to ensure everything works is more time-consuming and resource-intensive than making lots of small merges. 

The part I am struggling to imagine is when multiple people are making large changes at the exact same time. One large merge, in isolation, might be equivalent to many small ones in terms of work and cognitive load when it's the only change that can happen. What happens, though, when there are multiple sets of changes at once? The merge will be a nightmare if the code you relied on has now changed. Not to say this can't happen with small merges, but you'll catch this much earlier and resolve the communication breakdown within a day. 

Continuous Deployment (CD) is the practice of keeping production in line with the code in main at all times. They do not diverge. There are some practical notes to this: by doing so, you ensure your users get features and bug fixes as quickly as possible. It also holds the idea that something failing quickly is best, as it would have been a small, singular change rather than a multi-dimensional change. Therefore, making it much easier to fix going forward. It also avoids spending a lot of time further developing a feature if the MVP is a dud. 

Both are grouped together as they both increase the feedback you get on your code changes, which is good for iteration. 

The blocker in my mind is that these become much more valuable when you add multiple developers, but you don't have the full context. Therefore, I've not been exposed to the situations where these approaches really earn their keep. 

What has become valuable to me is ensuring that the source code always works on deployment. 

Which does mean I would now start to benefit from these practices. If I can now get into the position where deployment is automated in such a way that every test passes and all the steps pass, then that software should work to spec.

It is a valid approach to run all tests manually before deployment, although we will need to improve our test coverage. It's just that, do you really want to be doing this on every change? Do you want to risk making a mistake as well and missing a step? What if someone else wants to deploy something? Are you going to assume they will follow the steps? 

There is another key point in this: the repo should have all the testing steps clearly defined in code, making it clear to all what steps are required to run a full test suite before deployment.

For example:
1. Does the optimiser work end-to-end? 
2. Do all the API routes of the application work as expected?
3. Does the frontend load, and is it interactive with user input?
4. Do the required tables exist in the database, and do they have the correct schemas? 
	1. The creation of DB artefacts should be automatic.
	2. Migrations of previous DB artefacts should also be automatic on deployment - Atlas?

Then we need to balance this total test coverage with speed. Which then puts the question to you, the developer, which of these do you want to live with:
1. Wait on every commit for the tests to run? 
2. Wait on every push for the tests to run?
3. Wait on a PR to run all the tests for you before a merge, and ideally at the same time you are waiting for a review?

When set out in this way, you can see why doing it in the PR is often the best route; you tend to be waiting anyway for someone to review your code. Therefore, you can utilise that waiting time for deep testing rather than spending it while you are trying to develop.

Good Git and CI/CD practices do not pay off immediately. It does not pay a large amount when you are the sole contributor, and it has little to no value when you are not deploying to a live service. However, as each of these aspects becomes true, you start deploying to a live service, you work with other people or agents, and then the value of good practice in these areas starts to become increasingly obvious. 

This is not to say you can't be creative with how you do this; you can have time-based deployments still, you can have messy git histories in PRs and so on. However, you do need to ensure a good test coverage, and that this testing is run before deployment, then you can work out how best to do that.
