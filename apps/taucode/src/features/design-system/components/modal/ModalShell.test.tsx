// @vitest-environment jsdom
import { cleanup, fireEvent, render } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ModalShell } from "./ModalShell";

afterEach(cleanup);

describe("ModalShell", () => {
  it("renders root and card classes and handles backdrop click", () => {
    const onBackdropClick = vi.fn();
    const { baseElement } = render(
      <ModalShell
        className="custom-modal"
        cardClassName="custom-card"
        onBackdropClick={onBackdropClick}
        ariaLabel="My dialog"
      >
        <div>Modal content</div>
      </ModalShell>,
    );

    const modal = baseElement.querySelector(".ds-modal.custom-modal");
    const card = baseElement.querySelector(".ds-modal-card.custom-card");
    const backdrop = baseElement.querySelector(".ds-modal-backdrop");
    expect(modal).toBeTruthy();
    expect(card).toBeTruthy();
    expect(backdrop).toBeTruthy();
    if (!backdrop) {
      throw new Error("Expected modal backdrop");
    }
    expect(modal?.getAttribute("aria-label")).toBe("My dialog");
    fireEvent.click(backdrop);
    expect(onBackdropClick).toHaveBeenCalledTimes(1);
  });

  it("supports aria-labelledby and aria-describedby", () => {
    const { baseElement } = render(
      <ModalShell ariaLabelledBy="dialog-title" ariaDescribedBy="dialog-description">
        <h2 id="dialog-title">Dialog title</h2>
        <p id="dialog-description">Dialog description</p>
      </ModalShell>,
    );

    const modal = baseElement.querySelector(".ds-modal");
    expect(modal?.getAttribute("aria-labelledby")).toBe("dialog-title");
    expect(modal?.getAttribute("aria-describedby")).toBe("dialog-description");
  });
});
