#include "Menu.h"
#include "WorldIO.h"

#include <cctype>
#include <cmath>

namespace {

bool pointInRect(double px, double py, float x, float y, float w, float h) {
    return px >= x && px <= x + w && py >= y && py <= y + h;
}

} // namespace

Menu::Menu() {
    refreshWorldList();
}

void Menu::refreshWorldList() {
    worldNames_ = worldio::listWorldNames();
}

Menu::Result Menu::tick(GLFWwindow* window, Ui& ui, int windowWidth, int windowHeight, float currentTime) {
    Result result;
    (void)windowHeight;

    double mx = 0.0, my = 0.0;
    glfwGetCursorPos(window, &mx, &my);
    bool mouseDown = glfwGetMouseButton(window, GLFW_MOUSE_BUTTON_LEFT) == GLFW_PRESS;
    bool clicked = mouseDown && !prevMouseDown_;

    bool enterDown = glfwGetKey(window, GLFW_KEY_ENTER) == GLFW_PRESS || glfwGetKey(window, GLFW_KEY_KP_ENTER) == GLFW_PRESS;
    bool enterPressed = enterDown && !prevEnter_;
    bool escDown = glfwGetKey(window, GLFW_KEY_ESCAPE) == GLFW_PRESS;
    bool escPressed = escDown && !prevEscape_;
    bool backspaceDown = glfwGetKey(window, GLFW_KEY_BACKSPACE) == GLFW_PRESS;
    bool backspacePressed = backspaceDown && !prevBackspace_;

    const glm::vec4 titleColor(1.0f, 1.0f, 1.0f, 1.0f);
    const glm::vec4 textColor(0.9f, 0.9f, 0.9f, 1.0f);
    const glm::vec4 dimColor(0.6f, 0.6f, 0.6f, 1.0f);
    const glm::vec4 buttonColor(0.20f, 0.45f, 0.24f, 1.0f);
    const glm::vec4 buttonHoverColor(0.28f, 0.60f, 0.32f, 1.0f);
    const glm::vec4 rowColor(0.16f, 0.18f, 0.22f, 1.0f);
    const glm::vec4 rowHoverColor(0.24f, 0.27f, 0.33f, 1.0f);
    const glm::vec4 boxColor(0.12f, 0.13f, 0.16f, 1.0f);

    if (state_ == State::List) {
        const float titleScale = 5.0f;
        const std::string title = "PLUSCRAFT";
        ui.drawText(title, (windowWidth - ui.textWidth(title, titleScale)) / 2.0f, 40.0f, titleScale, titleColor);

        const float btnW = 280.0f, btnH = 50.0f;
        const float btnX = (windowWidth - btnW) / 2.0f;
        const float btnY = 130.0f;
        bool btnHover = pointInRect(mx, my, btnX, btnY, btnW, btnH);
        ui.drawRect(btnX, btnY, btnW, btnH, btnHover ? buttonHoverColor : buttonColor);

        const std::string btnLabel = "CREATE WORLD";
        const float lblScale = 2.5f;
        ui.drawText(btnLabel, btnX + (btnW - ui.textWidth(btnLabel, lblScale)) / 2.0f, btnY + (btnH - lblScale * 7.0f) / 2.0f, lblScale, titleColor);

        if (btnHover && clicked) {
            state_ = State::NamingWorld;
            nameBuffer_.clear();
        }

        float listTop = btnY + btnH + 30.0f;
        const std::string listLabel = "YOUR WORLDS";
        const float listLabelScale = 2.0f;
        ui.drawText(listLabel, (windowWidth - ui.textWidth(listLabel, listLabelScale)) / 2.0f, listTop, listLabelScale, dimColor);
        listTop += 24.0f;

        if (worldNames_.empty()) {
            const std::string msg = "NO SAVED WORLDS YET";
            const float msgScale = 2.0f;
            ui.drawText(msg, (windowWidth - ui.textWidth(msg, msgScale)) / 2.0f, listTop + 10.0f, msgScale, dimColor);
        } else {
            const float rowW = 280.0f, rowH = 40.0f, rowGap = 8.0f;
            const float rowX = (windowWidth - rowW) / 2.0f;
            for (size_t i = 0; i < worldNames_.size(); ++i) {
                float rowY = listTop + static_cast<float>(i) * (rowH + rowGap);
                bool rowHover = pointInRect(mx, my, rowX, rowY, rowW, rowH);
                ui.drawRect(rowX, rowY, rowW, rowH, rowHover ? rowHoverColor : rowColor);

                const float nameScale = 2.2f;
                ui.drawText(worldNames_[i], rowX + (rowW - ui.textWidth(worldNames_[i], nameScale)) / 2.0f,
                            rowY + (rowH - nameScale * 7.0f) / 2.0f, nameScale, textColor);

                if (rowHover && clicked) {
                    result.action = Action::LoadWorld;
                    result.value = worldio::pathForName(worldNames_[i]);
                }
            }
        }

        if (escPressed) {
            result.action = Action::Quit;
        }
    } else { // NamingWorld
        const std::string title = "NAME YOUR WORLD";
        const float titleScale = 3.5f;
        ui.drawText(title, (windowWidth - ui.textWidth(title, titleScale)) / 2.0f, 60.0f, titleScale, titleColor);

        const float boxW = 340.0f, boxH = 50.0f;
        const float boxX = (windowWidth - boxW) / 2.0f;
        const float boxY = 150.0f;
        ui.drawRect(boxX, boxY, boxW, boxH, boxColor);

        bool showCursor = std::fmod(currentTime, 1.0f) < 0.5f;
        std::string shown = nameBuffer_ + (showCursor ? "_" : "");
        const float nameScale = 2.6f;
        ui.drawText(shown, boxX + 14.0f, boxY + (boxH - nameScale * 7.0f) / 2.0f, nameScale, textColor);

        const std::string hint = "ENTER: CREATE    ESC: CANCEL";
        const float hintScale = 1.8f;
        ui.drawText(hint, (windowWidth - ui.textWidth(hint, hintScale)) / 2.0f, boxY + boxH + 24.0f, hintScale, dimColor);

        if (backspacePressed && !nameBuffer_.empty()) {
            nameBuffer_.pop_back();
        }
        if (enterPressed && !nameBuffer_.empty()) {
            result.action = Action::StartNewWorld;
            result.value = nameBuffer_;
            state_ = State::List;
        }
        if (escPressed) {
            state_ = State::List;
        }
    }

    prevMouseDown_ = mouseDown;
    prevEnter_ = enterDown;
    prevEscape_ = escDown;
    prevBackspace_ = backspaceDown;

    return result;
}

void Menu::onChar(unsigned int codepoint) {
    if (state_ != State::NamingWorld) {
        return;
    }
    if (nameBuffer_.size() >= 20) {
        return;
    }
    if (codepoint < 128) {
        char c = static_cast<char>(codepoint);
        if (std::isalnum(static_cast<unsigned char>(c)) || c == ' ' || c == '-' || c == '_') {
            nameBuffer_ += c;
        }
    }
}
