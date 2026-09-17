#pragma once

#include "Ui.h"

#define GLFW_INCLUDE_NONE
#include <GLFW/glfw3.h>

#include <string>
#include <vector>

// The pre-game screen: a "CREATE WORLD" button, a clickable list of
// existing saves (worlds/*.wrld), and a name-entry sub-screen for new
// worlds. Pure UI/input state machine - actually creating or loading
// the World is main.cpp's job, driven by what tick() returns.
class Menu {
public:
    Menu();

    enum class Action { None, StartNewWorld, LoadWorld, Quit };
    struct Result {
        Action action = Action::None;
        std::string value; // world name (StartNewWorld) or file path (LoadWorld)
    };

    // Call once per frame while the menu is showing: handles mouse and
    // (for name entry) keyboard input, and draws. windowWidth/Height
    // should be the current framebuffer size; currentTime is
    // glfwGetTime(), used to blink the text-entry cursor.
    Result tick(GLFWwindow* window, Ui& ui, int windowWidth, int windowHeight, float currentTime);

    // Wire this up to glfwSetCharCallback; only has an effect while
    // the name-entry sub-screen is showing.
    void onChar(unsigned int codepoint);

    // Called when the menu is about to be shown again (e.g. after
    // returning from a failed load) so its world list is current.
    void refreshWorldList();

private:
    enum class State { List, NamingWorld };
    State state_ = State::List;
    std::vector<std::string> worldNames_;
    std::string nameBuffer_;

    bool prevMouseDown_ = false;
    bool prevEnter_ = false;
    bool prevEscape_ = false;
    bool prevBackspace_ = false;
};
