using System;
using System.Drawing;
using System.Windows.Forms;
using PrayerTray.I18n;
using PrayerTray.Native;
using PrayerTray.Services;

namespace PrayerTray.UI;

/// <summary>Data-usage history dialog: today's totals, daily list (~90 days), and a reset button.</summary>
public class UsageForm : Form
{
    readonly Label _today = new() { AutoSize = true };
    readonly ListView _list = new()
    {
        View = View.Details,
        FullRowSelect = true,
        HeaderStyle = ColumnHeaderStyle.Nonclickable,
        BorderStyle = BorderStyle.FixedSingle,
        Dock = DockStyle.Fill,
    };

    public UsageForm()
    {
        Text = Strings.T("usage.title");
        if (Strings.IsRtl) { RightToLeft = RightToLeft.Yes; RightToLeftLayout = true; }
        FormBorderStyle = FormBorderStyle.FixedDialog;
        StartPosition = FormStartPosition.CenterScreen;
        MaximizeBox = false; MinimizeBox = false;
        TopMost = true; // above the always-on-top pill
        BackColor = Theme.Bg;
        ForeColor = Theme.Text;
        Font = new Font(Theme.Family, 9f);
        ClientSize = new Size(430, 430); // columns (364) + padding + vertical scrollbar

        _today.Font = new Font(Theme.Family, 10f, FontStyle.Bold);
        _today.ForeColor = Theme.Accent;
        _today.Margin = new Padding(4, 8, 4, 8);

        _list.BackColor = Theme.Panel;
        _list.ForeColor = Theme.Text;
        _list.Columns.Add(Strings.T("usage.date"), 110);
        _list.Columns.Add("↓ " + Strings.T("usage.down"), 82, HorizontalAlignment.Right);
        _list.Columns.Add("↑ " + Strings.T("usage.up"), 82, HorizontalAlignment.Right);
        _list.Columns.Add(Strings.T("usage.total"), 90, HorizontalAlignment.Right);

        var reset = new Button { Text = Strings.T("btn.resetUsage"), Width = 90, Height = 30 };
        var close = new Button { Text = Strings.T("btn.close"), Width = 90, Height = 30, DialogResult = DialogResult.Cancel };
        Style(reset); Style(close);
        reset.Click += OnReset;

        var buttons = new FlowLayoutPanel
        {
            FlowDirection = Strings.IsRtl ? FlowDirection.LeftToRight : FlowDirection.RightToLeft,
            Dock = DockStyle.Fill, AutoSize = true, Padding = new Padding(0, 8, 0, 0),
        };
        buttons.Controls.Add(close);
        buttons.Controls.Add(reset);

        var root = new TableLayoutPanel { Dock = DockStyle.Fill, ColumnCount = 1, RowCount = 3, Padding = new Padding(12) };
        root.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        root.RowStyles.Add(new RowStyle(SizeType.Percent, 100));
        root.RowStyles.Add(new RowStyle(SizeType.AutoSize));
        root.Controls.Add(_today, 0, 0);
        root.Controls.Add(_list, 0, 1);
        root.Controls.Add(buttons, 0, 2);
        Controls.Add(root);
        CancelButton = close;

        Refill();
    }

    protected override void OnHandleCreated(EventArgs e)
    {
        base.OnHandleCreated(e);
        Interop.TitleBar(Handle, Theme.Current.IsDark);
    }

    void Refill()
    {
        var (rx, tx) = DataUsage.Today();
        _today.Text = Strings.F("usage.today", DataUsage.Size(rx), DataUsage.Size(tx));

        _list.BeginUpdate();
        _list.Items.Clear();
        foreach (var (date, drx, dtx) in DataUsage.History())
            _list.Items.Add(new ListViewItem(new[]
                { date, DataUsage.Size(drx), DataUsage.Size(dtx), DataUsage.Size(drx + dtx) }));
        _list.EndUpdate();
    }

    void OnReset(object? sender, EventArgs e)
    {
        if (MessageBox.Show(this, Strings.T("usage.resetConfirm"), Strings.T("usage.title"),
                MessageBoxButtons.YesNo, MessageBoxIcon.Warning, MessageBoxDefaultButton.Button2,
                Strings.MsgOpts) != DialogResult.Yes) return;
        DataUsage.Reset();
        Refill();
    }

    static void Style(Button b)
    {
        b.FlatStyle = FlatStyle.Flat;
        b.FlatAppearance.BorderSize = 1;
        b.FlatAppearance.BorderColor = Theme.BgHover;
        b.BackColor = Theme.BgHover;
        b.ForeColor = Theme.Text;
        b.UseVisualStyleBackColor = false;
    }
}
