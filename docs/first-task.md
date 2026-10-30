# Your first Work task

Create a short summary from a text file. This example needs no coding and uses made-up notes, so you can practice without handing the app your important documents.

**Before you start:** taucode should open and answer a simple Chat message. If it does not, complete [installation and model setup](getting-started.md) first. This task uses the selected model and can incur your provider's normal API charges.

## 1. Create a practice folder

Use Finder on macOS or File Explorer on Windows to create a folder named **taucode-practice** inside Documents.

Inside it, create a plain-text file named **notes.txt**. On Windows you can use Notepad. On macOS you can use TextEdit; choose **Format → Make Plain Text** before saving so it does not become a rich-text document.

Copy this into the file and save it:

```text
Community garden meeting
The next meeting is Saturday at 10:00 AM.
Members should bring seeds and hand tools.
We still need to decide whether to buy new planters.
```

**Expected result:** you can open `notes.txt` from the practice folder and see those four lines.

Your folder should look like this:

```text
taucode-practice/
└── notes.txt
```

## 2. Add the folder to Work

1. In taucode, choose **Work** in the sidebar.
2. Select **Add space** and choose the **taucode-practice** folder you just made.
3. Create a new task in that space.

A **space** is simply the folder a Work task uses. A **task** is one conversation about work to do in that folder.

**Expected result:** the space appears in the sidebar, and the new task belongs to it. Check the folder before continuing; do not select your whole Documents folder for this exercise.

## 3. Give the task a clear request

Choose your configured model and review the displayed access setting. Start with **Ask** where an access choice is available. It lets the app pause for approval when its policy requires it.

Copy this into the task's message box:

```text
Read notes.txt in this space. Create outputs/summary.md with a short summary
of the meeting time, what members should bring, and the undecided question.
Use only information from notes.txt. Leave notes.txt unchanged.
```

Here is what the request means:

| Part | Why it helps |
| --- | --- |
| `Read notes.txt` | Names the input instead of asking the model to guess. |
| `outputs/summary.md` | Names the destination folder and output file. |
| Three specific facts | Makes the result easy to check. |
| Leave the source unchanged | States how to treat the original notes. |

`.md` means **Markdown**: a text document that can use simple headings and bullet points. You can read it in a text editor.

## 4. Run the task and watch its status

Select **Run now**. The task should move to **Running**, and its conversation shows progress.

If it says **Needs input**, open the task and read the question or approval request. For this exercise, the intended work is reading `notes.txt` and creating `outputs/summary.md` in the practice folder. If a request does not match that work, stop and investigate it instead of approving automatically.

You may see a plan listing steps. The wording depends on the model; it does not need to match this guide exactly.

**Expected result:** the task eventually finishes or displays a specific question/error. It should not require you to enable browser or desktop-app control for these local notes.

## 5. Open the output

Open the **Task** panel and look under **Outputs**. If another panel tab is selected, choose **Task**. Use **Open** or the reveal-in-file-manager control for the summary.

You can also look directly in the practice folder:

```text
taucode-practice/
├── notes.txt
└── outputs/
    └── summary.md
```

The output might use different wording, but it should contain all three facts:

- The meeting is Saturday at 10:00 AM.
- Members should bring seeds and hand tools.
- Buying new planters is still undecided.

**Check the actual file**, not only the agent's message saying it created one. Open `notes.txt` too and confirm that the original remains unchanged. The app may store additional working files; the tree above shows the input and deliverable you are checking.

## 6. Try a follow-up

In the same task, send:

```text
Rewrite outputs/summary.md as exactly three bullet points, one for each fact.
Keep notes.txt unchanged.
```

Open the file again and check the updated content. This is a follow-up to the same task, so the model can use the existing conversation as context.

## If the result is different

| What you see | What to do |
| --- | --- |
| The task stays Queued | It is waiting for a running-task slot. Check whether other tasks are active. |
| Needs input | Open the task and answer its question or review its approval request. |
| “File not found” | Check that `notes.txt` was saved in the selected space and has that exact name. |
| No output listed | Look in the actual folder and read the task's error/progress before trying again. |
| Extra or incorrect facts | Point out the mistake and ask for a correction based only on `notes.txt`. |
| The output will not open | Reveal it in the file manager and open `summary.md` with a text editor. |
| Authentication or model error | Return to [provider troubleshooting](providers.md#check-that-it-works). |

Use **Stop task** if work is taking an unexpected direction. Stop does not remove files already written; inspect them before retrying.

You have now practiced choosing a folder, giving a brief, reviewing progress, opening an output, and sending a follow-up. The [Work guide](work.md) explains document formats, queues, and recovery in more detail.
