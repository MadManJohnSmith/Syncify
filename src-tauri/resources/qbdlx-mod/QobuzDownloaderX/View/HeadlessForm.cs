using System;
using System.Runtime.InteropServices;
using System.Windows.Forms;

namespace QobuzDownloaderX.View
{
    public partial class HeadlessForm : Form
    {
        public HeadlessForm()
        {
            InitializeComponent();
        }

        public const int WM_NCLBUTTONDOWN = 0xA1;
        public const int HT_CAPTION = 0x2;

        [DllImportAttribute("user32.dll")]
        public static extern int SendMessage(IntPtr hWnd, int Msg, int wParam, int lParam);

        [DllImportAttribute("user32.dll")]
        public static extern bool ReleaseCapture();
    }
}